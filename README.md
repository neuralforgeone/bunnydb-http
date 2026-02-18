# bunnydb-http

HTTP client for Bunny.net Database SQL API pipeline endpoint:

`https://<db-id>.lite.bunnydb.net/v2/pipeline`

## Features

- `query`, `execute`, `batch`
- Positional (`?`) and named (`:name`) params
- Typed value model (`null`, integer, float, text, blob base64)
- Structured errors (`transport`, `http`, `pipeline`, `decode`)
- Configurable timeout and retry/backoff (`429` + `5xx`)

## Install

```toml
[dependencies]
bunnydb-http = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quickstart

```rust
use bunnydb_http::{BunnyDbClient, Params, Value};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("BUNNYDB_PIPELINE_URL")?;
    let token = std::env::var("BUNNYDB_TOKEN")?;

    let db = BunnyDbClient::new_bearer(url, token);

    db.execute(
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        (),
    ).await?;

    db.execute("INSERT INTO users (name) VALUES (?)", [Value::text("Kit")]).await?;

    let result = db.query(
        "SELECT id, name FROM users WHERE name = :name",
        Params::named([("name", Value::text("Kit"))]),
    ).await?;

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

## Notes

- Use `BunnyDbClient::new_bearer(url, token)` when you have only a token.
- Use `BunnyDbClient::new_raw_auth(url, authorization)` when you already have a full auth value.
- Retries are off by default (`max_retries = 0`).
- `batch` returns per-statement outcomes, including SQL errors with indexes.

## Optional features

- `tracing`: retry/debug logging hooks
- `raw-mode`: experimental raw response types
- `row-map`: experimental row helper types
- `baton-experimental`: experimental baton/session types
