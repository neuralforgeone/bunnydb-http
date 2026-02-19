use std::fmt;
use std::time::Duration;

use reqwest::{header, StatusCode};

// tokio::time::sleep is only available on non-WASM targets.
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep;

use crate::{
    decode::{build_execute_statement, decode_exec_result, decode_query_result},
    wire::{self, PipelineRequest, Request},
    BunnyDbError, ClientOptions, ExecResult, Params, QueryResult, Result, Statement,
    StatementOutcome,
};

/// Formats a database ID into the canonical pipeline URL.
///
/// Example: `"abc123"` → `"https://abc123.lite.bunnydb.net/v2/pipeline"`
pub fn db_id_to_pipeline_url(db_id: &str) -> String {
    format!("https://{}.lite.bunnydb.net/v2/pipeline", db_id.trim())
}

#[derive(Clone)]
/// HTTP client for Bunny.net Database SQL pipeline endpoint.
pub struct BunnyDbClient {
    http: reqwest::Client,
    pipeline_url: String,
    token: String,
    options: ClientOptions,
}

impl fmt::Debug for BunnyDbClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BunnyDbClient")
            .field("pipeline_url", &self.pipeline_url)
            .field("token", &"<redacted>")
            .field("options", &self.options)
            .finish()
    }
}

impl BunnyDbClient {
    /// Creates a client with a raw authorization header value.
    ///
    /// This is backward-compatible with previous versions where `token`
    /// was passed directly as `Authorization: <value>`.
    pub fn new(pipeline_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::new_raw_auth(pipeline_url, token)
    }

    /// Creates a client with a full raw authorization value.
    ///
    /// Example: `"Bearer <token>"` or any custom scheme.
    pub fn new_raw_auth(pipeline_url: impl Into<String>, authorization: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            pipeline_url: pipeline_url.into(),
            token: authorization.into(),
            options: ClientOptions::default(),
        }
    }

    /// Creates a client from a bearer token.
    ///
    /// If the token is missing the `Bearer ` prefix, it is added automatically.
    pub fn new_bearer(pipeline_url: impl Into<String>, token: impl AsRef<str>) -> Self {
        let authorization = normalize_bearer_authorization(token.as_ref());
        Self::new_raw_auth(pipeline_url, authorization)
    }

    /// Creates a client from a **Bunny Database ID** and a bearer token.
    ///
    /// The pipeline URL is derived automatically:
    /// `https://<db_id>.lite.bunnydb.net/v2/pipeline`
    ///
    /// This is the most ergonomic constructor when you know the database ID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bunnydb_http::BunnyDbClient;
    ///
    /// let db = BunnyDbClient::from_db_id("my-db-id", "my-token");
    /// ```
    pub fn from_db_id(db_id: impl AsRef<str>, token: impl AsRef<str>) -> Self {
        let url = db_id_to_pipeline_url(db_id.as_ref());
        Self::new_bearer(url, token)
    }

    /// Creates a client from environment variables.
    ///
    /// Reads:
    /// - `BUNNYDB_PIPELINE_URL` — full pipeline endpoint URL  
    ///   (e.g. `https://<id>.lite.bunnydb.net/v2/pipeline`)
    /// - `BUNNYDB_TOKEN` — access token (Bearer prefix optional)
    ///
    /// Returns an error if either variable is missing or empty.
    ///
    /// **Not available on `wasm32` targets** — environment variables do not
    /// exist in browser runtimes. Use [`BunnyDbClient::new_bearer`] or
    /// receive credentials from JavaScript via `wasm-bindgen`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bunnydb_http::BunnyDbClient;
    ///
    /// let db = BunnyDbClient::from_env().expect("missing BUNNYDB_* env vars");
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env() -> std::result::Result<Self, String> {
        let url = std::env::var("BUNNYDB_PIPELINE_URL")
            .map_err(|_| "missing BUNNYDB_PIPELINE_URL environment variable".to_owned())?;
        let token = std::env::var("BUNNYDB_TOKEN")
            .map_err(|_| "missing BUNNYDB_TOKEN environment variable".to_owned())?;
        if url.trim().is_empty() {
            return Err("BUNNYDB_PIPELINE_URL is set but empty".to_owned());
        }
        if token.trim().is_empty() {
            return Err("BUNNYDB_TOKEN is set but empty".to_owned());
        }
        Ok(Self::new_bearer(url, token))
    }

    /// Creates a client from a **database ID** read from the environment,
    /// combined with an access token also read from the environment.
    ///
    /// Reads:
    /// - `BUNNYDB_ID` — the database ID (e.g. `my-db-abc123`)
    /// - `BUNNYDB_TOKEN` — access token
    ///
    /// The pipeline URL is derived from the database ID automatically.
    ///
    /// **Not available on `wasm32` targets** — see [`BunnyDbClient::from_env`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bunnydb_http::BunnyDbClient;
    ///
    /// let db = BunnyDbClient::from_env_db_id().expect("missing BUNNYDB_ID / BUNNYDB_TOKEN");
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env_db_id() -> std::result::Result<Self, String> {
        let db_id = std::env::var("BUNNYDB_ID")
            .map_err(|_| "missing BUNNYDB_ID environment variable".to_owned())?;
        let token = std::env::var("BUNNYDB_TOKEN")
            .map_err(|_| "missing BUNNYDB_TOKEN environment variable".to_owned())?;
        if db_id.trim().is_empty() {
            return Err("BUNNYDB_ID is set but empty".to_owned());
        }
        if token.trim().is_empty() {
            return Err("BUNNYDB_TOKEN is set but empty".to_owned());
        }
        Ok(Self::from_db_id(db_id, token))
    }

    /// Applies client options such as timeout and retry behavior.
    pub fn with_options(mut self, opts: ClientOptions) -> Self {
        self.options = opts;
        self
    }

    /// Executes a query statement and returns rows.
    pub async fn query<P: Into<Params>>(&self, sql: &str, params: P) -> Result<QueryResult> {
        let result = self.run_single(sql, params.into(), true).await?;
        decode_query_result(result)
    }

    /// Executes a statement and returns execution metadata.
    pub async fn execute<P: Into<Params>>(&self, sql: &str, params: P) -> Result<ExecResult> {
        let result = self.run_single(sql, params.into(), false).await?;
        decode_exec_result(result)
    }

    /// Sends multiple statements in one pipeline request.
    ///
    /// SQL errors at statement level are returned as
    /// [`StatementOutcome::SqlError`] instead of failing the entire batch.
    pub async fn batch<I>(&self, statements: I) -> Result<Vec<StatementOutcome>>
    where
        I: IntoIterator<Item = Statement>,
    {
        let statements: Vec<Statement> = statements.into_iter().collect();
        let mut requests = Vec::with_capacity(statements.len() + 1);
        let mut wants_rows = Vec::with_capacity(statements.len());

        for statement in statements {
            let stmt =
                build_execute_statement(&statement.sql, statement.params, statement.want_rows)?;
            requests.push(Request::Execute { stmt });
            wants_rows.push(statement.want_rows);
        }

        requests.push(Request::Close {});
        let payload = PipelineRequest { requests };
        let response = self.send_pipeline_with_retry(&payload).await?;

        let expected = wants_rows.len() + 1;
        if response.results.len() != expected {
            return Err(BunnyDbError::Decode(format!(
                "result count mismatch: expected {expected}, got {}",
                response.results.len()
            )));
        }

        let mut results = response.results.into_iter();
        let mut outcomes = Vec::with_capacity(wants_rows.len());

        for (index, want_rows) in wants_rows.into_iter().enumerate() {
            let result = results.next().ok_or_else(|| {
                BunnyDbError::Decode(format!("missing execute result at index {index}"))
            })?;
            outcomes.push(Self::decode_statement_outcome(result, index, want_rows)?);
        }

        let close_index = outcomes.len();
        let close = results.next().ok_or_else(|| {
            BunnyDbError::Decode(format!("missing close result at index {close_index}"))
        })?;
        Self::ensure_close_success(close, close_index)?;

        Ok(outcomes)
    }

    async fn run_single(
        &self,
        sql: &str,
        params: Params,
        want_rows: bool,
    ) -> Result<wire::ExecuteResult> {
        let execute_stmt = build_execute_statement(sql, params, want_rows)?;
        let payload = PipelineRequest {
            requests: vec![Request::Execute { stmt: execute_stmt }, Request::Close {}],
        };
        let response = self.send_pipeline_with_retry(&payload).await?;

        if response.results.len() != 2 {
            return Err(BunnyDbError::Decode(format!(
                "result count mismatch: expected 2, got {}",
                response.results.len()
            )));
        }

        let mut iter = response.results.into_iter();
        let execute = iter
            .next()
            .ok_or_else(|| BunnyDbError::Decode("missing execute result".to_owned()))?;
        let close = iter
            .next()
            .ok_or_else(|| BunnyDbError::Decode("missing close result".to_owned()))?;

        let execute_result = Self::into_execute_result(execute, 0)?;
        Self::ensure_close_success(close, 1)?;
        Ok(execute_result)
    }

    async fn send_pipeline_with_retry(
        &self,
        payload: &PipelineRequest,
    ) -> Result<wire::PipelineResponse> {
        let mut attempt = 0usize;
        loop {
            // Build the request. On WASM, reqwest uses AbortController for
            // timeout; the `.timeout()` method is available on both targets.
            let response = self
                .http
                .post(&self.pipeline_url)
                .header(header::AUTHORIZATION, &self.token)
                .header(header::CONTENT_TYPE, "application/json")
                .timeout(Duration::from_millis(self.options.timeout_ms))
                .json(payload)
                .send()
                .await;

            match response {
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.map_err(BunnyDbError::Transport)?;

                    if !status.is_success() {
                        if self.should_retry_status(status) && attempt < self.options.max_retries {
                            self.wait_before_retry(attempt).await;
                            attempt += 1;
                            continue;
                        }

                        return Err(BunnyDbError::Http {
                            status: status.as_u16(),
                            body,
                        });
                    }

                    return serde_json::from_str::<wire::PipelineResponse>(&body).map_err(|err| {
                        BunnyDbError::Decode(format!(
                            "invalid pipeline response JSON: {err}; body: {body}"
                        ))
                    });
                }
                Err(err) => {
                    if self.should_retry_transport(&err) && attempt < self.options.max_retries {
                        self.wait_before_retry(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(BunnyDbError::Transport(err));
                }
            }
        }
    }

    fn decode_statement_outcome(
        result: wire::PipelineResult,
        request_index: usize,
        want_rows: bool,
    ) -> Result<StatementOutcome> {
        match result.kind.as_str() {
            "ok" => {
                let execute_result = Self::into_execute_result(result, request_index)?;
                if want_rows {
                    Ok(StatementOutcome::Query(decode_query_result(
                        execute_result,
                    )?))
                } else {
                    Ok(StatementOutcome::Exec(decode_exec_result(execute_result)?))
                }
            }
            "error" => {
                let error = result.error.ok_or_else(|| {
                    BunnyDbError::Decode(format!(
                        "missing error payload for request {request_index}"
                    ))
                })?;
                Ok(StatementOutcome::SqlError {
                    request_index,
                    message: error.message,
                    code: error.code,
                })
            }
            other => Err(BunnyDbError::Decode(format!(
                "unknown pipeline result type '{other}' at request {request_index}"
            ))),
        }
    }

    fn into_execute_result(
        result: wire::PipelineResult,
        request_index: usize,
    ) -> Result<wire::ExecuteResult> {
        match result.kind.as_str() {
            "ok" => {
                let response = result.response.ok_or_else(|| {
                    BunnyDbError::Decode(format!(
                        "missing response payload for request {request_index}"
                    ))
                })?;
                if response.kind != "execute" {
                    return Err(BunnyDbError::Decode(format!(
                        "expected execute response at request {request_index}, got '{}'",
                        response.kind
                    )));
                }
                response.result.ok_or_else(|| {
                    BunnyDbError::Decode(format!(
                        "missing execute result payload at request {request_index}"
                    ))
                })
            }
            "error" => {
                let error = result.error.ok_or_else(|| {
                    BunnyDbError::Decode(format!(
                        "missing error payload for request {request_index}"
                    ))
                })?;
                Err(BunnyDbError::Pipeline {
                    request_index,
                    message: error.message,
                    code: error.code,
                })
            }
            other => Err(BunnyDbError::Decode(format!(
                "unknown pipeline result type '{other}' at request {request_index}"
            ))),
        }
    }

    fn ensure_close_success(result: wire::PipelineResult, request_index: usize) -> Result<()> {
        match result.kind.as_str() {
            "ok" => {
                let response = result.response.ok_or_else(|| {
                    BunnyDbError::Decode(format!(
                        "missing close response payload for request {request_index}"
                    ))
                })?;
                if response.kind != "close" {
                    return Err(BunnyDbError::Decode(format!(
                        "expected close response at request {request_index}, got '{}'",
                        response.kind
                    )));
                }
                Ok(())
            }
            "error" => {
                let error = result.error.ok_or_else(|| {
                    BunnyDbError::Decode(format!(
                        "missing error payload for close request {request_index}"
                    ))
                })?;
                Err(BunnyDbError::Pipeline {
                    request_index,
                    message: error.message,
                    code: error.code,
                })
            }
            other => Err(BunnyDbError::Decode(format!(
                "unknown pipeline result type '{other}' at request {request_index}"
            ))),
        }
    }

    fn should_retry_status(&self, status: StatusCode) -> bool {
        matches!(
            status,
            StatusCode::TOO_MANY_REQUESTS
                | StatusCode::INTERNAL_SERVER_ERROR
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT
        )
    }

    fn should_retry_transport(&self, err: &reqwest::Error) -> bool {
        err.is_timeout()
            || err.is_request()
            || err.is_body()
            // is_connect() is not available on wasm32 targets (no TCP)
            || {
                #[cfg(not(target_arch = "wasm32"))]
                { err.is_connect() }
                #[cfg(target_arch = "wasm32")]
                { false }
            }
    }

    /// Waits before the next retry attempt.
    ///
    /// On native targets: exponential backoff sleep via `tokio::time::sleep`.
    /// On WASM targets: no-op — edge functions prefer fast failure over
    /// sleeping, and `tokio::time::sleep` is not available.
    async fn wait_before_retry(&self, attempt: usize) {
        let exp = attempt.min(16) as u32;
        let multiplier = 1u64 << exp;
        let delay_ms = self.options.retry_backoff_ms.saturating_mul(multiplier);

        #[cfg(feature = "tracing")]
        tracing::debug!("retrying pipeline request after {} ms", delay_ms);

        #[cfg(not(target_arch = "wasm32"))]
        sleep(Duration::from_millis(delay_ms)).await;

        // WASM: no sleep implementation — suppress unused variable warning.
        #[cfg(target_arch = "wasm32")]
        let _ = delay_ms;
    }
}

fn normalize_bearer_authorization(token: &str) -> String {
    let trimmed = token.trim();
    let prefix = trimmed.get(..7);
    if prefix.is_some_and(|value| value.eq_ignore_ascii_case("bearer ")) {
        trimmed.to_owned()
    } else {
        format!("Bearer {trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_bearer_authorization, BunnyDbClient};

    #[test]
    fn normalize_bearer_adds_prefix_when_missing() {
        assert_eq!(
            normalize_bearer_authorization("abc123"),
            "Bearer abc123".to_owned()
        );
    }

    #[test]
    fn normalize_bearer_keeps_existing_prefix() {
        assert_eq!(
            normalize_bearer_authorization("bEaReR abc123"),
            "bEaReR abc123".to_owned()
        );
    }

    #[test]
    fn debug_redacts_authorization_value() {
        let client = BunnyDbClient::new_raw_auth("https://db/v2/pipeline", "secret-token");
        let debug = format!("{client:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret-token"));
    }
}
