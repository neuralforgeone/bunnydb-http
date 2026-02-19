# Architecture — bunnydb-http

This document describes the internal structure of the `bunnydb-http` crate, the
responsibilities of each module, how data flows from a user call down to the
wire, and design decisions that were made along the way.

---

## Module Map

```
bunnydb-http/src/
├── lib.rs          ← Public API surface, re-exports
├── client.rs       ← BunnyDbClient — constructors, query/execute/batch, retry
├── decode.rs       ← statement builder + response decoder
├── wire.rs         ← JSON wire types for /v2/pipeline
├── params.rs       ← Params, Statement — user-facing parameter builders
├── value.rs        ← Value — typed SQL values (null/integer/float/text/blob)
├── types.rs        ← QueryResult, ExecResult, Col, StatementOutcome
├── options.rs      ← ClientOptions (timeout, retries, backoff)
├── error.rs        ← BunnyDbError enum
│
├── baton.rs        ← [feature: baton-experimental] session baton type
├── raw.rs          ← [feature: raw-mode] raw wire response passthrough
└── row_map.rs      ← [feature: row-map] row-to-map helper
```

---

## Data Flow

```
User code
    │
    │  db.query(sql, params) / db.execute(...) / db.batch([...])
    ▼
client.rs  ──  run_single() / batch()
    │
    │  decode.rs: build_execute_statement()
    │   • Params → wire::Stmt { sql, args, named_args }
    │   • validates float finiteness
    ▼
wire.rs
    PipelineRequest { requests: [Execute { stmt }, Close] }
    │
    │  serde_json serialised → HTTP POST /v2/pipeline
    │  Authorization: Bearer <token>
    ▼
Bunny.net Database API
    │
    │  JSON response body
    ▼
wire.rs (deserialize)
    PipelineResponse { results: [PipelineResult, ...] }
    │
    │  decode.rs: decode_query_result() / decode_exec_result()
    │   • wire::Col  → types::Col
    │   • wire rows  → Vec<Vec<Value>>
    │   • telemetry  → rows_read, rows_written, query_duration_ms
    ▼
User code receives QueryResult / ExecResult / Vec<StatementOutcome>
```

---

## Client Construction Hierarchy

```
from_env()          ← reads BUNNYDB_PIPELINE_URL + BUNNYDB_TOKEN
from_env_db_id()    ← reads BUNNYDB_ID + BUNNYDB_TOKEN → db_id_to_pipeline_url()
from_db_id(id, tok) ← db_id_to_pipeline_url(id) + new_bearer()
new_bearer(url, tok) ← normalize_bearer_authorization() + new_raw_auth()
new_raw_auth(url, auth) ← lowest-level constructor
new(url, tok)       ← alias for new_raw_auth (backward compat)
```

`db_id_to_pipeline_url(db_id)` converts a database ID to the canonical
pipeline endpoint:

```
"abc123" → "https://abc123.lite.bunnydb.net/v2/pipeline"
```

---

## Retry Logic

`send_pipeline_with_retry` implements a simple exponential-backoff retry:

```
attempt 0: send
  on 429 / 5xx / transport error → wait(backoff_ms * 2^attempt) → attempt 1
  on success → return
  on hard error (4xx other than 429) → return error immediately
```

| Status | Retried? |
|--------|----------|
| 429 Too Many Requests | ✅ |
| 500 Internal Server Error | ✅ |
| 502 Bad Gateway | ✅ |
| 503 Service Unavailable | ✅ |
| 504 Gateway Timeout | ✅ |
| 4xx (others) | ❌ |
| Transport timeout | ✅ |
| Connection error | ✅ |

---

## Parameter Encoding

### Positional (`?`)

```rust
// User
db.query("SELECT * FROM t WHERE id = ?", [Value::integer(1)]).await?;

// Wire: args array
{ "sql": "SELECT * FROM t WHERE id = ?", "args": [{ "type": "integer", "value": "1" }] }
```

### Named (`:name`, `$name`, `@name`)

```rust
// User
db.query("SELECT * FROM t WHERE name = :name",
    Params::named([("name", Value::text("Kit"))])).await?;

// Wire: named_args array
{ "sql": "...", "named_args": [{ "name": "name", "value": { "type": "text", "value": "Kit" } }] }
```

---

## Value Types

| Rust helper | Wire `type` | Notes |
|---|---|---|
| `Value::null()` | `"null"` | |
| `Value::integer(n)` | `"integer"` | value serialised as string |
| `Value::float(f)` | `"float"` | panics / errors on NaN/Inf |
| `Value::text(s)` | `"text"` | |
| `Value::blob(b)` | `"blob"` | base64 encoded |

---

## Optional Features

| Feature | Module | Description |
|---|---|---|
| `tracing` | client.rs | Debug tracing for retry events |
| `raw-mode` | raw.rs | Raw `PipelineResponse` passthrough |
| `row-map` | row_map.rs | `QueryResult::to_map()` helper |
| `baton-experimental` | baton.rs | Session baton / interactive session type |

---

## Telemetry Fields

Every `QueryResult` and `ExecResult` carries server-side telemetry:

| Field | Type | Description |
|---|---|---|
| `rows_read` | `Option<u64>` | Rows scanned during query |
| `rows_written` | `Option<u64>` | Rows mutated |
| `query_duration_ms` | `Option<u64>` | Server-side execution time |

These map directly to the `rows_read`, `rows_written`, and
`query_duration_ms` fields in the `/v2/pipeline` response envelope.

---

## Design Decisions

### Why HTTP-only (not libSQL native driver)?

The libSQL native protocol uses WebSockets and a custom binary protocol. An
HTTP-only crate is:

- Zero platform dependencies (works in WASM, containers, serverless, etc.)
- Easier to audit and proxy
- Compatible with any Bunny region without driver-level routing

### Why not `From<String>` for errors?

`BunnyDbError` uses `thiserror` with concrete variants so callers can match
on specific error kinds without parsing strings.

### Why `reqwest` with `rustls-tls`?

`native-tls` requires platform TLS libraries which complicate cross-compilation.
`rustls` is pure Rust and works everywhere Rust does.
