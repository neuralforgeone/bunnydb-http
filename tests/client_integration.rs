use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use bunnydb_http::{
    BunnyDbClient, BunnyDbError, ClientOptions, Statement, StatementOutcome, Value,
};
use serde_json::{json, Value as JsonValue};

#[derive(Clone)]
struct MockResponse {
    status: StatusCode,
    body: JsonValue,
    delay: Duration,
}

impl MockResponse {
    fn json(status: StatusCode, body: JsonValue) -> Self {
        Self {
            status,
            body,
            delay: Duration::from_millis(0),
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

#[derive(Clone)]
struct MockState {
    responses: Arc<Mutex<VecDeque<MockResponse>>>,
    hits: Arc<AtomicUsize>,
}

async fn pipeline_handler(State(state): State<MockState>, _body: String) -> impl IntoResponse {
    state.hits.fetch_add(1, Ordering::SeqCst);

    let response = {
        let mut queue = state
            .responses
            .lock()
            .expect("response queue mutex must not be poisoned");
        queue.pop_front().unwrap_or_else(|| {
            MockResponse::json(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": "no mock response available"}),
            )
        })
    };

    if !response.delay.is_zero() {
        tokio::time::sleep(response.delay).await;
    }

    (response.status, Json(response.body))
}

struct TestServer {
    base_url: String,
    hits: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TestServer {
    fn pipeline_url(&self) -> String {
        format!("{}/v2/pipeline", self.base_url)
    }
}

async fn spawn_server(responses: Vec<MockResponse>) -> TestServer {
    let state = MockState {
        responses: Arc::new(Mutex::new(responses.into())),
        hits: Arc::new(AtomicUsize::new(0)),
    };

    let app = Router::new()
        .route("/v2/pipeline", post(pipeline_handler))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("must bind test listener");
    let address = listener.local_addr().expect("must have local addr");
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("mock server must run");
    });

    TestServer {
        base_url: format!("http://{address}"),
        hits: state.hits,
        task,
    }
}

fn query_pipeline_body() -> JsonValue {
    json!({
        "results": [
            {
                "type": "ok",
                "response": {
                    "type": "execute",
                    "result": {
                        "cols": [
                            { "name": "id", "decltype": "INTEGER" },
                            { "name": "name", "decltype": "TEXT" }
                        ],
                        "rows": [
                            [
                                { "type": "integer", "value": "1" },
                                { "type": "text", "value": "Kit" }
                            ]
                        ],
                        "affected_row_count": 0
                    }
                }
            },
            {
                "type": "ok",
                "response": {
                    "type": "close"
                }
            }
        ]
    })
}

fn execute_pipeline_body(affected_rows: u64, last_insert_rowid: Option<&str>) -> JsonValue {
    json!({
        "results": [
            {
                "type": "ok",
                "response": {
                    "type": "execute",
                    "result": {
                        "affected_row_count": affected_rows,
                        "last_insert_rowid": last_insert_rowid
                    }
                }
            },
            {
                "type": "ok",
                "response": { "type": "close" }
            }
        ]
    })
}

#[tokio::test]
async fn query_returns_rows_and_cols() {
    let server = spawn_server(vec![MockResponse::json(
        StatusCode::OK,
        query_pipeline_body(),
    )])
    .await;
    let db = BunnyDbClient::new(server.pipeline_url(), "token");

    let result = db
        .query(
            "SELECT id, name FROM users WHERE name = ?",
            [Value::text("Kit")],
        )
        .await
        .expect("query must succeed");

    assert_eq!(result.cols.len(), 2);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], Value::Integer(1));
    assert_eq!(result.rows[0][1], Value::Text("Kit".to_owned()));
    assert_eq!(server.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn execute_returns_affected_row_count_and_last_rowid() {
    let server = spawn_server(vec![MockResponse::json(
        StatusCode::OK,
        execute_pipeline_body(1, Some("42")),
    )])
    .await;
    let db = BunnyDbClient::new(server.pipeline_url(), "token");

    let result = db
        .execute("INSERT INTO users (name) VALUES (?)", [Value::text("Kit")])
        .await
        .expect("execute must succeed");

    assert_eq!(result.affected_row_count, 1);
    assert_eq!(result.last_insert_rowid, Some(42));
    assert_eq!(server.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn batch_returns_statement_level_sql_error_without_failing_request() {
    let body = json!({
        "results": [
            {
                "type": "ok",
                "response": {
                    "type": "execute",
                    "result": { "affected_row_count": 1, "last_insert_rowid": "1" }
                }
            },
            {
                "type": "error",
                "error": {
                    "message": "near \"INSER\": syntax error",
                    "code": "SQLITE_ERROR"
                }
            },
            {
                "type": "ok",
                "response": {
                    "type": "execute",
                    "result": {
                        "cols": [
                            { "name": "cnt", "decltype": "INTEGER" }
                        ],
                        "rows": [
                            [
                                { "type": "integer", "value": "1" }
                            ]
                        ]
                    }
                }
            },
            {
                "type": "ok",
                "response": { "type": "close" }
            }
        ]
    });
    let server = spawn_server(vec![MockResponse::json(StatusCode::OK, body)]).await;
    let db = BunnyDbClient::new(server.pipeline_url(), "token");

    let outcomes = db
        .batch([
            Statement::execute("INSERT INTO users(name) VALUES (?)", [Value::text("A")]),
            Statement::execute("INSER INTO users(name) VALUES (?)", [Value::text("B")]),
            Statement::query("SELECT COUNT(*) AS cnt FROM users", ()),
        ])
        .await
        .expect("batch must succeed with per-statement errors");

    assert_eq!(outcomes.len(), 3);
    assert!(matches!(outcomes[0], StatementOutcome::Exec(_)));
    assert!(matches!(
        outcomes[1],
        StatementOutcome::SqlError {
            request_index: 1,
            ..
        }
    ));
    assert!(matches!(outcomes[2], StatementOutcome::Query(_)));
}

#[tokio::test]
async fn retries_on_retryable_http_status() {
    let server = spawn_server(vec![
        MockResponse::json(StatusCode::INTERNAL_SERVER_ERROR, json!({"error": "boom"})),
        MockResponse::json(StatusCode::OK, execute_pipeline_body(2, Some("7"))),
    ])
    .await;

    let db = BunnyDbClient::new(server.pipeline_url(), "token").with_options(ClientOptions {
        timeout_ms: 1_000,
        max_retries: 1,
        retry_backoff_ms: 1,
    });

    let result = db
        .execute("UPDATE users SET name = ?", [Value::text("Renamed")])
        .await
        .expect("request must succeed after retry");

    assert_eq!(result.affected_row_count, 2);
    assert_eq!(server.hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn request_timeout_surfaces_transport_error() {
    let server = spawn_server(vec![MockResponse::json(
        StatusCode::OK,
        execute_pipeline_body(1, Some("1")),
    )
    .with_delay(Duration::from_millis(150))])
    .await;

    let db = BunnyDbClient::new(server.pipeline_url(), "token").with_options(ClientOptions {
        timeout_ms: 20,
        max_retries: 0,
        retry_backoff_ms: 1,
    });

    let err = db
        .execute("DELETE FROM users", ())
        .await
        .expect_err("request must timeout");

    match err {
        BunnyDbError::Transport(inner) => assert!(inner.is_timeout()),
        _ => panic!("expected transport timeout error"),
    }
}

#[tokio::test]
async fn query_pipeline_sql_error_in_execute_is_top_level_error() {
    let body = json!({
        "results": [
            {
                "type": "error",
                "error": {
                    "message": "no such table: users",
                    "code": "SQLITE_ERROR"
                }
            },
            {
                "type": "ok",
                "response": { "type": "close" }
            }
        ]
    });
    let server = spawn_server(vec![MockResponse::json(StatusCode::OK, body)]).await;
    let db = BunnyDbClient::new(server.pipeline_url(), "token");

    let err = db
        .query("SELECT * FROM users", ())
        .await
        .expect_err("query must fail");

    match err {
        BunnyDbError::Pipeline { request_index, .. } => assert_eq!(request_index, 0),
        _ => panic!("expected pipeline error"),
    }
}
