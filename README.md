# bunnydb-http

[![crates.io](https://img.shields.io/crates/v/bunnydb-http.svg)](https://crates.io/crates/bunnydb-http)
[![docs.rs](https://docs.rs/bunnydb-http/badge.svg)](https://docs.rs/bunnydb-http)
[![CI](https://github.com/neuralforgeone/bunnydb-http/actions/workflows/ci.yml/badge.svg)](https://github.com/neuralforgeone/bunnydb-http/actions/workflows/ci.yml)
[![WASM](https://img.shields.io/badge/target-wasm32--unknown--unknown-blueviolet)](https://webassembly.org)

Async Rust client for the Bunny.net Database SQL pipeline API —
works on **native** (tokio) and **WebAssembly** (`wasm32-unknown-unknown`,
Bunny Edge Scripts).

Target endpoint format:

`https://<db-id>.lite.bunnydb.net/v2/pipeline`

## Highlights

- Async API with `query`, `execute`, `batch`
- Positional (`?`) and named (`:name`) parameters
- Typed values: `null`, integer, float, text, blob base64
- Structured error model: transport, HTTP, pipeline, decode
- Configurable timeout and retry/backoff for `429` and `5xx`
- Query telemetry fields (`rows_read`, `rows_written`, `query_duration_ms`)
- ✅ **`wasm32-unknown-unknown`** — runs inside Bunny Edge Scripts via the browser `fetch` API

## Installation

### Native (server, Docker, Magic Container)

```toml
[dependencies]
bunnydb-http = "0.3"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### WebAssembly (Bunny Edge Script)

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
bunnydb-http = "0.3"       # reqwest uses fetch API automatically on wasm32
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
```

No extra feature flags — the crate detects `wasm32-unknown-unknown` at
compile time and swaps `tokio` for the browser runtime automatically.

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

| Feature | Description |
|---|---|
| `tracing` | retry/debug tracing hooks |
| `raw-mode` | experimental raw response types |
| `row-map` | experimental row mapping helpers |
| `baton-experimental` | experimental baton/session types |

## Platform Support

| Target | Status | Notes |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ✅ | Primary target, full tokio |
| `aarch64-unknown-linux-gnu` | ✅ | ARM64, Docker, Magic Containers |
| `x86_64-apple-darwin` | ✅ | macOS native |
| `wasm32-unknown-unknown` | ✅ | **Bunny Edge Scripts**, browser, Deno |

On `wasm32-unknown-unknown`:

- `reqwest` uses the browser `fetch` API (no TLS layer needed)
- `tokio` is not linked — the WASM runtime drives the event loop
- `from_env()` / `from_env_db_id()` are not available (no `std::env` in browsers)
- Retry backoff sleep is a no-op — edge functions prefer fast failures
- `BunnyDbClient::new_bearer()`, `from_db_id()`, `query`, `execute`, `batch` work identically

## Bunny Edge Scripting & Magic Containers

### Option 1 — Magic Container (pure Rust, native binary)

[Bunny Magic Containers](https://docs.bunny.net/database/connect/magic-containers)
run a Docker workload co-located with the database — full Rust ecosystem,
no WASM needed.

1. Open the Bunny dashboard → **Database** → your DB → **Access** → generate a token.
2. In your Magic Container environment variables:

```
BUNNYDB_PIPELINE_URL = https://<your-db-id>.lite.bunnydb.net/v2/pipeline
BUNNYDB_TOKEN        = <your-token>
```

3. In your Rust code:

```rust
let db = BunnyDbClient::from_env().expect("missing BUNNYDB_* env vars");
```

---

### Option 2 — Edge Script (Rust → WASM) 🆕

Compile your Rust logic to `wasm32-unknown-unknown` and deploy it as a
**Bunny Edge Script**. The same `BunnyDbClient` API, same type safety —
running at the CDN edge PoP nearest to your users.

```
Bunny CDN edge PoP
  └── edge/main.ts           tiny TypeScript host (~30 lines)
        ↕ wasm-bindgen
  └── src/lib.rs             your Rust logic compiled to .wasm
        └── bunnydb-http     reqwest → browser fetch API
              └── BunnyDB /v2/pipeline
```

#### Rust side (`src/lib.rs`)

```rust
use bunnydb_http::{BunnyDbClient, Value};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct EdgeHandler {
    db: BunnyDbClient,
}

#[wasm_bindgen]
impl EdgeHandler {
    #[wasm_bindgen(constructor)]
    pub fn new(pipeline_url: &str, token: &str) -> Self {
        Self { db: BunnyDbClient::new_bearer(pipeline_url, token) }
    }

    /// Query users and return JSON string.
    pub async fn get_users(&self) -> Result<String, String> {
        let result = self.db
            .query("SELECT id, name FROM users ORDER BY id DESC LIMIT 50", ())
            .await
            .map_err(|e| e.to_string())?;

        // Build a JSON array of rows
        let rows: Vec<String> = result.rows.iter().map(|row| {
            let id   = match &row[0] { bunnydb_http::Value::Integer(n) => n.to_string(), v => format!("{v:?}") };
            let name = match &row[1] { bunnydb_http::Value::Text(s) => s.clone(), v => format!("{v:?}") };
            format!(r#"{{"id":{id},"name":"{name}"}}"#)
        }).collect();

        Ok(format!("[{}]", rows.join(",")))
    }

    /// Insert a user and return affected row count.
    pub async fn create_user(&self, name: String, email: String) -> Result<String, String> {
        let result = self.db
            .execute(
                "INSERT INTO users (name, email) VALUES (?, ?)",
                [Value::text(name), Value::text(email)],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(format!(r#"{{"affected":{},"id":{:?}}}"#,
            result.affected_row_count, result.last_insert_rowid))
    }
}
```

#### Edge Script host (`edge/main.ts`)

```typescript
import * as BunnySDK from "https://esm.sh/@bunny.net/edgescript-sdk@0.12.0";
import process from "node:process";
import init, { EdgeHandler } from "./pkg/my_handler.js";  // wasm-pack output

// Load the .wasm binary once at cold start
await init(fetch(process.env.WASM_URL!));

// Create Rust handler — credentials from Bunny env vars
const handler = new EdgeHandler(process.env.DB_URL!, process.env.DB_TOKEN!);

BunnySDK.net.http.serve(async (req: Request): Promise<Response> => {
  const url = new URL(req.url);

  if (req.method === "GET" && url.pathname === "/users") {
    const json = await handler.get_users();
    return new Response(json, { headers: { "Content-Type": "application/json" } });
  }

  if (req.method === "POST" && url.pathname === "/users") {
    const { name, email } = await req.json();
    const result = await handler.create_user(name, email);
    return new Response(result, { status: 201, headers: { "Content-Type": "application/json" } });
  }

  return new Response("not found", { status: 404 });
});
```

#### Build & deploy

```bash
# 1. Install wasm-pack
cargo install wasm-pack

# 2. Compile Rust → WASM
wasm-pack build --target bundler --release
# → pkg/my_handler_bg.wasm  (~150–250 KB optimized)
# → pkg/my_handler.js       (wasm-bindgen glue)

# 3. Upload .wasm to Bunny Storage
curl -X PUT "https://storage.bunnycdn.com/<zone>/my_handler_bg.wasm" \
  -H "AccessKey: <key>" --data-binary @pkg/my_handler_bg.wasm

# 4. Set env vars in Edge Script dashboard:
#    WASM_URL  = https://your-cdn.b-cdn.net/my_handler_bg.wasm
#    DB_URL    = https://<db-id>.lite.bunnydb.net/v2/pipeline
#    DB_TOKEN  = <your-token>
```

A complete, ready-to-deploy example is in [`examples/wasm-edge/`](examples/wasm-edge/).

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
