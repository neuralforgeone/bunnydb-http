use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use bunnydb_http::{BunnyDbClient, BunnyDbError, Params, Statement, StatementOutcome, Value};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SecretsFile {
    #[serde(rename = "BUNNYDB_PIPELINE_URL")]
    bunnydb_pipeline_url: Option<String>,
    #[serde(rename = "BUNNYDB_TOKEN")]
    bunnydb_token: Option<String>,
    #[serde(rename = "BUNNY_DATABASE_URL")]
    bunny_database_url: Option<String>,
    #[serde(rename = "BUNNY_DATABASE_AUTH_TOKEN")]
    bunny_database_auth_token: Option<String>,
}

fn to_pipeline_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/v2/pipeline") {
        return trimmed.to_owned();
    }
    if let Some(host) = trimmed.strip_prefix("libsql://") {
        return format!("https://{host}/v2/pipeline");
    }
    format!("{trimmed}/v2/pipeline")
}

fn to_authorization_token(token: String) -> String {
    if token.contains(' ') {
        token
    } else {
        format!("Bearer {token}")
    }
}

fn load_live_credentials() -> Result<(String, String), String> {
    if let (Ok(pipeline_url), Ok(token)) = (
        std::env::var("BUNNYDB_PIPELINE_URL"),
        std::env::var("BUNNYDB_TOKEN"),
    ) {
        return Ok((pipeline_url, to_authorization_token(token)));
    }

    let content = fs::read_to_string("secrets.json").map_err(|_| {
        "BUNNYDB_PIPELINE_URL/BUNNYDB_TOKEN env or secrets.json is required".to_owned()
    })?;
    let parsed: SecretsFile = serde_json::from_str(&content)
        .map_err(|err| format!("secrets.json could not be parsed: {err}"))?;

    let pipeline_url = parsed
        .bunnydb_pipeline_url
        .or_else(|| parsed.bunny_database_url.map(|url| to_pipeline_url(&url)))
        .ok_or_else(|| {
            "missing BUNNYDB_PIPELINE_URL or BUNNY_DATABASE_URL in secrets.json".to_owned()
        })?;
    let token = parsed
        .bunnydb_token
        .or(parsed.bunny_database_auth_token)
        .ok_or_else(|| {
            "missing BUNNYDB_TOKEN or BUNNY_DATABASE_AUTH_TOKEN in secrets.json".to_owned()
        })?;

    Ok((pipeline_url, to_authorization_token(token)))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after epoch")
        .as_millis()
}

#[tokio::test]
async fn live_roundtrip_and_batch_error_propagation() {
    let (pipeline_url, token) = match load_live_credentials() {
        Ok(values) => values,
        Err(_) => {
            eprintln!("skipping live test: credentials not found in env or secrets.json");
            return;
        }
    };

    let db = BunnyDbClient::new(pipeline_url, token);
    let table = format!("users_live_{}", unique_suffix());

    db.execute(
        &format!("CREATE TABLE IF NOT EXISTS {table} (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"),
        (),
    )
    .await
    .expect("table creation must succeed");

    db.execute(
        &format!("INSERT INTO {table} (name) VALUES (?)"),
        [Value::text("Kit")],
    )
    .await
    .expect("insert must succeed");

    let query = db
        .query(
            &format!("SELECT id, name FROM {table} WHERE name = :name"),
            Params::named([("name", Value::text("Kit"))]),
        )
        .await
        .expect("query must succeed");
    assert_eq!(query.rows.len(), 1);

    let outcomes = db
        .batch([
            Statement::execute(
                format!("INSERT INTO {table} (name) VALUES (?)"),
                [Value::text("BatchA")],
            ),
            Statement::execute(
                format!("INSER INTO {table} (name) VALUES (?)"),
                [Value::text("BatchB")],
            ),
            Statement::query(format!("SELECT COUNT(*) FROM {table}"), ()),
        ])
        .await
        .expect("batch must return outcomes");

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

    let cleanup_result = db
        .execute(&format!("DROP TABLE IF EXISTS {table}"), ())
        .await;
    if let Err(BunnyDbError::Pipeline { message, .. }) = cleanup_result {
        panic!("cleanup failed with pipeline error: {message}");
    }
}
