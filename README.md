# bunnydb-http

[![crates.io](https://img.shields.io/crates/v/bunnydb-http.svg)](https://crates.io/crates/bunnydb-http)
[![docs.rs](https://docs.rs/bunnydb-http/badge.svg)](https://docs.rs/bunnydb-http)
[![CI](https://github.com/neuralforgeone/bunnydb-http/actions/workflows/ci.yml/badge.svg)](https://github.com/neuralforgeone/bunnydb-http/actions/workflows/ci.yml)

Async Rust client for Bunny.net Database SQL pipeline API.

Target endpoint format:

`https://<db-id>.lite.bunnydb.net/v2/pipeline`

## Highlights

- Async API with `query`, `execute`, `batch`
- Positional (`?`) and named (`:name`) parameters
- Typed values: `null`, integer, float, text, blob base64
- Structured error model: transport, HTTP, pipeline, decode
- Configurable timeout and retry/backoff for `429` and `5xx`
- Query telemetry fields (`rows_read`, `rows_written`, `query_duration_ms`)

## Installation

```toml
[dependencies]
bunnydb-http = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick Start

```rust
use bunnydb_http::{BunnyDbClient, Params, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipeline_url = std::env::var("BUNNYDB_PIPELINE_URL")?;
    let token = std::env::var("BUNNYDB_TOKEN")?;

    let db = BunnyDbClient::new_bearer(pipeline_url, token);

    db.execute(
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        (),
    )
    .await?;

    db.execute("INSERT INTO users (name) VALUES (?)", [Value::text("Kit")])
        .await?;

    let result = db
        .query(
            "SELECT id, name FROM users WHERE name = :name",
            Params::named([("name", Value::text("Kit"))]),
        )
        .await?;

    println!(
        "rows={}, rows_read={:?}, rows_written={:?}, duration_ms={:?}",
        result.rows.len(),
        result.rows_read,
        result.rows_written,
        result.query_duration_ms
    );

    Ok(())
}
```

## Authentication and Endpoint

- `BunnyDbClient::new_bearer(url, token)`:
  Pass only the token, crate sends `Authorization: Bearer <token>`.
- `BunnyDbClient::new_raw_auth(url, authorization)`:
  Pass full authorization value directly.
- `BunnyDbClient::new(url, token)`:
  Backward-compatible raw constructor.

`url` must point to the pipeline endpoint (`.../v2/pipeline`).

## Parameters

Positional:

```rust
db.query("SELECT * FROM users WHERE id = ?", [Value::integer(1)]).await?;
```

Named:

```rust
db.query(
    "SELECT * FROM users WHERE name = :name",
    Params::named([("name", Value::text("Kit"))]),
)
.await?;
```

## Batch Semantics

`batch` returns per-statement outcomes and does not fail the full request for SQL-level statement errors.

```rust
use bunnydb_http::{Statement, StatementOutcome, Value};

let outcomes = db.batch([
    Statement::execute("INSERT INTO users(name) VALUES (?)", [Value::text("A")]),
    Statement::execute("INSER INTO users(name) VALUES (?)", [Value::text("B")]),
    Statement::query("SELECT COUNT(*) FROM users", ()),
]).await?;

for outcome in outcomes {
    match outcome {
        StatementOutcome::Exec(exec) => println!("affected={}", exec.affected_row_count),
        StatementOutcome::Query(query) => println!("rows={}", query.rows.len()),
        StatementOutcome::SqlError { request_index, message, .. } => {
            eprintln!("sql error at {request_index}: {message}");
        }
    }
}
```

## Timeout and Retry

```rust
use bunnydb_http::{BunnyDbClient, ClientOptions};

let db = BunnyDbClient::new_bearer(pipeline_url, token).with_options(ClientOptions {
    timeout_ms: 10_000,
    max_retries: 2,
    retry_backoff_ms: 250,
});
```

Defaults:

- `timeout_ms = 10_000`
- `max_retries = 0`
- `retry_backoff_ms = 250`

## Error Model

- `BunnyDbError::Transport(reqwest::Error)`
- `BunnyDbError::Http { status, body }`
- `BunnyDbError::Pipeline { request_index, message, code }`
- `BunnyDbError::Decode(String)`

## Optional Features

- `tracing`: retry/debug tracing hooks
- `raw-mode`: experimental raw response types
- `row-map`: experimental row mapping helpers
- `baton-experimental`: experimental baton/session types

## Testing

Run all tests:

```bash
cargo test
```

Live integration test reads credentials in this order:

- Environment:
  `BUNNYDB_PIPELINE_URL` and `BUNNYDB_TOKEN`
- Local file fallback:
  `secrets.json` with either
  `BUNNYDB_PIPELINE_URL` + `BUNNYDB_TOKEN`
  or `BUNNY_DATABASE_URL` + `BUNNY_DATABASE_AUTH_TOKEN`

`secrets.json` is excluded from packaging.

## Publishing (crates.io)

```bash
cargo fmt --all
cargo test
cargo publish --dry-run
cargo publish
```

## MSRV

Rust `1.75`

## License

MIT
