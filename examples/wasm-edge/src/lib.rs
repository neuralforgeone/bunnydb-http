//! # BunnyDB Edge Handler
//!
//! A pure-Rust WebAssembly module that runs inside a **Bunny Edge Script**.
//!
//! This crate compiles to `wasm32-unknown-unknown` and exposes async handlers
//! via `wasm-bindgen`. The TypeScript edge-script host (`edge/main.ts`) loads
//! the `.wasm` binary, passes the BunnyDB credentials, and calls these
//! functions for each HTTP request.
//!
//! ## Build
//!
//! ```bash
//! wasm-pack build --target bundler --release
//! # outputs pkg/bunnydb_edge_handler_bg.wasm and glue JS
//! ```
//!
//! ## Architecture
//!
//! ```
//! Bunny Edge Script (Deno/V8)
//!   └── edge/main.ts          ← TypeScript host, loads .wasm
//!         ↕ wasm-bindgen glue
//!   └── src/lib.rs            ← Rust handler (this file, compiled to WASM)
//!         └── bunnydb-http    ← HTTP pipeline client (reqwest → fetch API)
//!               └── BunnyDB /v2/pipeline
//! ```

use bunnydb_http::{BunnyDbClient, Value};
use wasm_bindgen::prelude::*;

// ── Handler struct ──────────────────────────────────────────────────────────

/// WASM-exported handle to a connected BunnyDB client.
///
/// Construct via [`BunnyEdgeHandler::new`], then call handler methods
/// from the TypeScript edge script.
#[wasm_bindgen]
pub struct BunnyEdgeHandler {
    db: BunnyDbClient,
}

#[wasm_bindgen]
impl BunnyEdgeHandler {
    /// Creates a new handler connected to the given BunnyDB pipeline endpoint.
    ///
    /// In a Bunny Edge Script, pass the environment variables injected by the
    /// Bunny dashboard:
    ///
    /// ```typescript
    /// import process from "node:process";
    /// const handler = new BunnyEdgeHandler(process.env.DB_URL, process.env.DB_TOKEN);
    /// ```
    ///
    /// `pipeline_url` must be the full endpoint:
    /// `https://<db-id>.lite.bunnydb.net/v2/pipeline`
    ///
    /// If you only have the DB id, use [`BunnyEdgeHandler::from_db_id`].
    #[wasm_bindgen(constructor)]
    pub fn new(pipeline_url: &str, token: &str) -> BunnyEdgeHandler {
        BunnyEdgeHandler {
            db: BunnyDbClient::new_bearer(pipeline_url, token),
        }
    }

    /// Creates a handler using just the **database ID** — the pipeline URL
    /// is derived automatically.
    ///
    /// ```typescript
    /// const handler = BunnyEdgeHandler.from_db_id("my-db-abc123", process.env.DB_TOKEN);
    /// ```
    pub fn from_db_id(db_id: &str, token: &str) -> BunnyEdgeHandler {
        BunnyEdgeHandler {
            db: BunnyDbClient::from_db_id(db_id, token),
        }
    }

    // ── Query helpers ───────────────────────────────────────────────────────

    /// Runs a raw SQL SELECT and returns all rows as a JSON string.
    ///
    /// Columns and rows are serialized to:
    /// ```json
    /// {
    ///   "cols": ["id", "name"],
    ///   "rows": [[1, "Kit"], [2, "Lane"]],
    ///   "rows_read": 2,
    ///   "query_duration_ms": 0.5
    /// }
    /// ```
    pub async fn query_json(&self, sql: String) -> Result<String, String> {
        let result = self.db.query(&sql, ()).await.map_err(|e| e.to_string())?;

        let col_names: Vec<&str> = result.cols.iter().map(|c| c.name.as_str()).collect();
        let rows: Vec<Vec<serde_json::Value>> = result
            .rows
            .iter()
            .map(|row| row.iter().map(value_to_json).collect())
            .collect();

        let payload = serde_json::json!({
            "cols": col_names,
            "rows": rows,
            "rows_read": result.rows_read,
            "rows_written": result.rows_written,
            "query_duration_ms": result.query_duration_ms,
        });

        serde_json::to_string(&payload).map_err(|e| e.to_string())
    }

    /// Executes a SQL statement (INSERT / UPDATE / DELETE / DDL).
    ///
    /// Returns a JSON string:
    /// ```json
    /// { "affected_row_count": 1, "last_insert_rowid": 42, "rows_written": 1 }
    /// ```
    pub async fn execute_json(&self, sql: String) -> Result<String, String> {
        let result = self.db.execute(&sql, ()).await.map_err(|e| e.to_string())?;

        let payload = serde_json::json!({
            "affected_row_count": result.affected_row_count,
            "last_insert_rowid": result.last_insert_rowid,
            "rows_written": result.rows_written,
        });

        serde_json::to_string(&payload).map_err(|e| e.to_string())
    }

    /// Executes a parameterized INSERT with positional `?` placeholders.
    ///
    /// `values_json` must be a JSON array of primitives, e.g. `[1, "Kit", null]`.
    ///
    /// ```typescript
    /// await handler.insert_one("INSERT INTO users(name) VALUES (?)", "[\"Kit\"]");
    /// ```
    pub async fn insert_one(&self, sql: String, values_json: String) -> Result<String, String> {
        let raw: Vec<serde_json::Value> =
            serde_json::from_str(&values_json).map_err(|e| e.to_string())?;
        let params: Vec<Value> = raw.iter().map(json_to_value).collect();

        let result = self
            .db
            .execute(&sql, params)
            .await
            .map_err(|e| e.to_string())?;

        let payload = serde_json::json!({
            "affected_row_count": result.affected_row_count,
            "last_insert_rowid": result.last_insert_rowid,
        });

        serde_json::to_string(&payload).map_err(|e| e.to_string())
    }
}

// ── Value conversion helpers ────────────────────────────────────────────────

/// Converts a `bunnydb_http::Value` to a `serde_json::Value` suitable for
/// JSON serialisation in the WASM response.
fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Integer(n) => serde_json::json!(n),
        Value::Float(f) => serde_json::json!(f),
        Value::Text(s) => serde_json::json!(s),
        Value::BlobBase64(b) => serde_json::json!(b),
    }
}

/// Converts a JSON primitive to a `bunnydb_http::Value` for parameterized queries.
fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::null(),
        serde_json::Value::Bool(b) => Value::integer(i64::from(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::float(f)
            } else {
                Value::text(n.to_string())
            }
        }
        serde_json::Value::String(s) => Value::text(s.clone()),
        other => Value::text(other.to_string()),
    }
}
