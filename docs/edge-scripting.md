# Edge Scripting — bunnydb-rs

This document explains how to use **bunnydb-rs** from within
**Bunny Edge Scripts** and **Magic Container** deployments.

> **What is Bunny Edge Scripting?**  
> Edge Scripts are lightweight serverless JavaScript / TypeScript functions
> that run at Bunny's global PoPs, close to the user. They can be Standalone
> (handle HTTP requests directly) or Middleware (intercept CDN traffic).  
> See: <https://docs.bunny.net/scripting>

---

## Overview

Bunny Database provides two integration paths for edge/container workloads:

| Deployment type | Credential delivery | Recommended constructor |
|---|---|---|
| **Edge Script** (JS/TS) | `process.env.DB_URL` + `process.env.DB_TOKEN` | native `@libsql/client/web` |
| **Magic Container** (Docker) | env vars injected at deploy time | `BunnyDbClient::from_env()` |
| **Any Rust binary** | env vars, `.env`, secrets manager | `BunnyDbClient::from_env()` or `from_env_db_id()` |

> **Note:** Edge Scripts run V8/Deno — they use the `@libsql/client/web`
> TypeScript SDK. This crate targets Rust binaries running in Magic
> Containers, native services, or any Rust async runtime.

---

## Connecting from a Magic Container (Rust)

Bunny Magic Containers are Docker containers deployed and co-located with the
database in the same region to minimise latency.

### Step 1 — Generate an access token

1. Open [dash.bunny.net](https://dash.bunny.net) → **Database** → your DB → **Access**.
2. Click **Generate token** → choose **Full Access** or **Read Only**.
3. Copy the token.

### Step 2 — Set environment variables in your container

In the Bunny dashboard, under your Magic Container app's **Environment
Variables**, add:

```
BUNNYDB_PIPELINE_URL = https://<your-db-id>.lite.bunnydb.net/v2/pipeline
BUNNYDB_TOKEN        = <your-access-token>
```

Alternatively, if you prefer to set only the database ID:

```
BUNNYDB_ID    = <your-db-id>
BUNNYDB_TOKEN = <your-access-token>
```

### Step 3 — Use `from_env()` in your Rust code

```rust
use bunnydb_http::BunnyDbClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Reads BUNNYDB_PIPELINE_URL + BUNNYDB_TOKEN automatically
    let db = BunnyDbClient::from_env()
        .expect("Set BUNNYDB_PIPELINE_URL and BUNNYDB_TOKEN");

    // Or, if you only set BUNNYDB_ID + BUNNYDB_TOKEN:
    // let db = BunnyDbClient::from_env_db_id()?;

    let result = db.query("SELECT * FROM users", ()).await?;
    println!("{} users", result.rows.len());
    Ok(())
}
```

### Alternative: `from_db_id` with token from a secret

```rust
use bunnydb_http::BunnyDbClient;

// DB ID is known at compile time; token comes from an env var / secret
let db_id = "my-db-abc123";
let token = std::env::var("BUNNYDB_TOKEN")?;
let db = BunnyDbClient::from_db_id(db_id, token);
```

---

## Connecting from a Bunny Edge Script (TypeScript)

Edge Scripts use the official `@libsql/client/web` package. The Bunny
dashboard injects the credentials as environment variables automatically
when you link a database to a script.

### Setup

1. Go to your **Edge Script** → **Settings** → **Database**.
2. Select the database and click **Add to script**.
3. The dashboard injects `DB_URL` and `DB_TOKEN` automatically.

### Code

```typescript
import { createClient } from "@libsql/client/web";
import process from "node:process";

const client = createClient({
  url: process.env.DB_URL,
  authToken: process.env.DB_TOKEN,
});

// Standalone script entry point
export default async function handler(request: Request): Promise<Response> {
  const result = await client.execute("SELECT * FROM users");
  return Response.json(result.rows);
}
```

> The `@libsql/client/web` package uses the same `/v2/pipeline` HTTP
> protocol under the hood — the same protocol this Rust crate implements.

---

## Wire Protocol Reference

Both the Rust crate and the TypeScript SDK communicate with the same
`/v2/pipeline` REST endpoint.

### Endpoint

```
POST https://<db-id>.lite.bunnydb.net/v2/pipeline
Authorization: Bearer <token>
Content-Type: application/json
```

### Request envelope

```json
{
  "requests": [
    {
      "type": "execute",
      "stmt": {
        "sql": "SELECT * FROM users WHERE id = ?",
        "args": [{ "type": "integer", "value": "1" }]
      }
    },
    { "type": "close" }
  ]
}
```

Every request **must** end with a `close` entry.

### Named parameters

```json
{
  "requests": [
    {
      "type": "execute",
      "stmt": {
        "sql": "SELECT * FROM users WHERE name = :name",
        "named_args": [
          { "name": "name", "value": { "type": "text", "value": "Kit" } }
        ]
      }
    },
    { "type": "close" }
  ]
}
```

Supported prefixes: `:name`, `$name`, `@name`.

### Value types

| `type` string | Rust `Value` constructor | Notes |
|---|---|---|
| `"null"` | `Value::null()` | |
| `"integer"` | `Value::integer(n)` | JSON value is a **string** |
| `"float"` | `Value::float(f)` | Must be finite |
| `"text"` | `Value::text(s)` | |
| `"blob"` | `Value::blob(b)` | Base64-encoded bytes |

### Response envelope

```json
{
  "baton": null,
  "base_url": null,
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
          "affected_row_count": 0,
          "last_insert_rowid": null,
          "replication_index": "1",
          "rows_read": 1,
          "rows_written": 0,
          "query_duration_ms": 0
        }
      }
    },
    {
      "type": "ok",
      "response": { "type": "close" }
    }
  ]
}
```

### Error response

```json
{
  "results": [
    {
      "type": "error",
      "error": {
        "message": "no such table: missing",
        "code": "SQLITE_ERROR"
      }
    }
  ]
}
```

---

## Authentication

### Token types

| Type | Permissions | Recommended for |
|---|---|---|
| **Full Access** | Read + Write | Application backends |
| **Read Only** | SELECT only | Public-facing read APIs |

### HTTP header format

```
Authorization: Bearer <token>
```

The crate's `new_bearer()`, `from_db_id()`, `from_env()`, and
`from_env_db_id()` constructors all add the `Bearer ` prefix automatically
if it is missing.

---

## Replication & Latency

Bunny Database separates storage from compute:

- **Storage**: Toronto (CA) or Frankfurt (DE)
- **Replica read regions**: globally distributed PoPs

Writes are routed to the primary region; reads are served from the
nearest replica. When using Magic Containers, the container and the
database replica are co-located in the same region, keeping latency
in the single-digit millisecond range.

---

## Interactive Sessions (Baton)

The pipeline protocol supports a `baton` field for interactive sessions —
a way to chain multiple pipeline requests in the same logical connection
(useful for transactions).

The `baton-experimental` feature exposes the `Baton` type. Full
interactive-session support is planned for a future version.

---

## Observability

### Telemetry in query results

Every query response includes server-side metrics:

```rust
let result = db.query("SELECT * FROM users", ()).await?;

if let Some(ms) = result.query_duration_ms {
    println!("server-side duration: {ms} ms");
}
if let Some(read) = result.rows_read {
    println!("rows scanned: {read}");
}
```

### Dashboard metrics

The Bunny dashboard exposes per-database metrics:

| Metric | Description |
|---|---|
| Rows read | Rows scanned by SELECT queries |
| Rows written | Rows mutated by INSERT/UPDATE/DELETE |
| Latency | End-to-end request latency histogram |
| Query count | Total requests per time window |
| Database size | Storage used |

Access via: **Database** → your DB → **Metrics** tab.

---

## Environment Variable Reference

| Variable | Used by | Description |
|---|---|---|
| `BUNNYDB_PIPELINE_URL` | `from_env()` | Full pipeline URL |
| `BUNNYDB_TOKEN` | `from_env()`, `from_env_db_id()` | Bearer token |
| `BUNNYDB_ID` | `from_env_db_id()` | Database ID (URL derived automatically) |
| `DB_URL` | Edge Script (TS) | Full `libsql://` URL |
| `DB_TOKEN` | Edge Script (TS) | Bearer token |

> Bunny automatically injects `DB_URL` and `DB_TOKEN` for linked databases
> in Edge Scripts and Magic Containers.  
> For standalone Rust binaries, set `BUNNYDB_*` variables manually.

---

## External Links

- [Bunny Database — Introduction](https://docs.bunny.net/database/index)
- [Bunny Edge Scripting — Introduction](https://docs.bunny.net/scripting)
- [Bunny Database — SQL API](https://docs.bunny.net/database/connect/sql-api)
- [Bunny Database — Auth & Access](https://docs.bunny.net/database/connect/authorization)
- [Bunny Database — Replication](https://docs.bunny.net/database/replication)
- [Bunny Database — Durability & Consistency](https://docs.bunny.net/database/durability-and-consistency)
- [Bunny Edge Scripting — Database](https://docs.bunny.net/database/connect/scripting)
- [Bunny Magic Containers — Database](https://docs.bunny.net/database/connect/magic-containers)
