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
bunnydb-http = "0.2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Client Construction

Choose the constructor that fits your deployment:

| Constructor | When to use |
|---|---|
| `BunnyDbClient::from_env()` | 12-factor apps, Docker, CI: reads `BUNNYDB_PIPELINE_URL` + `BUNNYDB_TOKEN` |
| `BunnyDbClient::from_env_db_id()` | Edge scripts / containers: reads `BUNNYDB_ID` + `BUNNYDB_TOKEN` |
| `BunnyDbClient::from_db_id(id, tok)` | Known DB ID, token from config |
| `BunnyDbClient::new_bearer(url, tok)` | Full URL + bearer token |
| `BunnyDbClient::new_raw_auth(url, auth)` | Full URL + custom auth header |

```toml
# Recommended defaults for production
BUNNYDB_PIPELINE_URL=https://<db-id>.lite.bunnydb.net/v2/pipeline
BUNNYDB_TOKEN=<your-token>
```

## Quick Start

### Option A — environment variables (recommended)

The most autonomous setup: set env vars once, no URL construction in code.

```rust
use bunnydb_http::BunnyDbClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reads BUNNYDB_PIPELINE_URL + BUNNYDB_TOKEN automatically
    let db = BunnyDbClient::from_env().expect("missing BUNNYDB_* env vars");

    db.execute(
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        (),
    ).await?;

    let result = db
        .query(
            "SELECT id, name FROM users WHERE name = :name",
            bunnydb_http::Params::named([("name", bunnydb_http::Value::text("Kit"))]),
        )
        .await?;

    println!("rows={}", result.rows.len());
    Ok(())
}
```

### Option B — database ID + token

```rust
use bunnydb_http::BunnyDbClient;

// URL is derived automatically from the ID
let db = BunnyDbClient::from_db_id("my-db-abc123", "my-token");
```

### Option C — explicit URL

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

- `BunnyDbClient::from_env()`:  
  Reads `BUNNYDB_PIPELINE_URL` and `BUNNYDB_TOKEN` from environment. Ideal for 12-factor apps, Docker, CI.
- `BunnyDbClient::from_env_db_id()`:  
  Reads `BUNNYDB_ID` and `BUNNYDB_TOKEN`. URL constructed automatically.
- `BunnyDbClient::from_db_id(db_id, token)`:  
  Provide a database ID; URL constructed as `https://<db_id>.lite.bunnydb.net/v2/pipeline`.
- `BunnyDbClient::new_bearer(url, token)`:  
  Pass the full pipeline URL and token. `Bearer ` prefix added automatically.
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

## Bunny Edge Scripting & Magic Containers

`bunnydb-http` is designed to work seamlessly inside
[Bunny Magic Containers](https://docs.bunny.net/database/connect/magic-containers)
(Docker workloads co-located with the database) and any other deployment
that uses environment variables for configuration.

### Magic Container setup

1. Open the Bunny dashboard → **Database** → your DB → **Access**.
2. Generate tokens → copy the token.
3. In your Magic Container app settings, add:

```
BUNNYDB_PIPELINE_URL = https://<your-db-id>.lite.bunnydb.net/v2/pipeline
BUNNYDB_TOKEN        = <your-token>
```

4. In your Rust code:

```rust
let db = BunnyDbClient::from_env().expect("missing BUNNYDB_* env vars");
```

Or, if you only set `BUNNYDB_ID`:

```rust
let db = BunnyDbClient::from_env_db_id().expect("missing BUNNYDB_ID / BUNNYDB_TOKEN");
```

### Bunny Edge Scripts (TypeScript / JS)

Edge Scripts run in the Bunny CDN JavaScript runtime (V8/Deno). They use
the official `@libsql/client/web` package, which uses the same `/v2/pipeline`
protocol this crate implements.

```typescript
import { createClient } from "@libsql/client/web";
import process from "node:process";

const client = createClient({
  url: process.env.DB_URL,       // injected by Bunny automatically
  authToken: process.env.DB_TOKEN, // injected by Bunny automatically
});

export default async function handler(req: Request): Promise<Response> {
  const result = await client.execute("SELECT * FROM users");
  return Response.json(result.rows);
}
```

See [docs/edge-scripting.md](docs/edge-scripting.md) for the full wire
protocol reference, authentication details, and replication notes.

## GUI Client (Example)

This repo includes a desktop GUI example built with `eframe/egui`.

Run it:

```bash
cargo run --example gui
```

The GUI supports:

- Query / Execute / Batch modes
- Bearer or raw authorization mode
- JSON params:
  `[]` for positional, `{}` for named
- Batch JSON format:

```json
[
  { "kind": "execute", "sql": "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)" },
  { "kind": "execute", "sql": "INSERT INTO users (name) VALUES (?)", "params": ["Kit"] },
  { "kind": "query", "sql": "SELECT id, name FROM users", "params": [] }
]
```

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

## Documentation

| Document | Description |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Module map, data flow, design decisions |
| [docs/edge-scripting.md](docs/edge-scripting.md) | Edge Scripting, Magic Containers, wire protocol reference |

## MSRV

Rust `1.75`

## License

MIT
