# bunnydb-edge-handler

**Pure-Rust WebAssembly handler for Bunny Edge Scripts.**  
Uses `bunnydb-http` to query BunnyDB directly from the edge — no Node.js,
no TypeScript DB logic, no extra hop.

```
Your users → Bunny CDN Edge PoP
                ↓
        edge/main.ts       (tiny TS host, ~30 lines)
                ↓  wasm-bindgen
        src/lib.rs    ←── bunnydb-http crate
                ↓  reqwest (fetch API in WASM)
          BunnyDB /v2/pipeline
```

---

## Prerequisites

```bash
# Install wasm-pack
cargo install wasm-pack

# Or via the installer script
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

---

## Build

```bash
# From examples/wasm-edge/
wasm-pack build --target bundler --release
```

This produces `pkg/` containing:
- `bunnydb_edge_handler_bg.wasm`  — the Rust logic (~150KB optimized)
- `bunnydb_edge_handler.js`       — wasm-bindgen JS glue
- `bunnydb_edge_handler.d.ts`     — TypeScript types

---

## Deploy to Bunny Edge Scripting

### 1. Upload the WASM binary to Bunny Storage

```bash
# Upload to your Bunny Storage zone
curl -X PUT \
  "https://storage.bunnycdn.com/<zone>/bunnydb_edge_handler_bg.wasm" \
  -H "AccessKey: <storage-api-key>" \
  -H "Content-Type: application/wasm" \
  --data-binary @pkg/bunnydb_edge_handler_bg.wasm
```

### 2. Set environment variables in your Edge Script

In [dash.bunny.net](https://dash.bunny.net) → **Edge Scripts** → your script
→ **Environment Variables**:

```
WASM_URL   = https://your-cdn.b-cdn.net/bunnydb_edge_handler_bg.wasm
DB_URL     = https://<db-id>.lite.bunnydb.net/v2/pipeline
DB_TOKEN   = <your-access-token>
```

Or use `DB_ID` instead of `DB_URL`:
```
DB_ID      = <your-db-id>
DB_TOKEN   = <your-access-token>
```

### 3. Deploy the edge script

Copy `edge/main.ts` into your Bunny Edge Script editor, or connect the
repository via GitHub integration for automatic deploys.

---

## API

| Method | Path | Description |
|---|---|---|
| `GET` | `/users` | Query all users (handled in Rust) |
| `POST` | `/users` | Insert a user `{ "name": "...", "email": "..." }` |
| `GET` | `/health` | Liveness check — returns `{"status":"ok","runtime":"rust+wasm"}` |

---

## Customising the Rust handler

Edit `src/lib.rs` to add your own BunnyDB logic:

```rust
// In BunnyEdgeHandler impl:
pub async fn get_active_users(&self) -> Result<String, String> {
    let result = self.db
        .query("SELECT * FROM users WHERE active = ?", [Value::integer(1)])
        .await
        .map_err(|e| e.to_string())?;

    // ... serialize result
}
```

Then rebuild:
```bash
wasm-pack build --target bundler --release
```

---

## Size optimization

The `[profile.release]` section in `Cargo.toml` is configured for minimum
`.wasm` size (`opt-level = "z"`, LTO enabled). A typical build is **~150–250KB**
compressed, well within Bunny's 1MB script limit.

---

## Why Rust instead of TypeScript for DB logic?

| | TypeScript | Rust/WASM |
|---|---|---|
| Type safety | ✅ (runtime errors possible) | ✅ (compile-time guarantees) |
| Error handling | `try/catch` | `Result<T, E>` |
| Performance | JS engine JIT | Near-native WASM |
| SQL parameter safety | Manual | Typed `Value` enum |
| Reuse crate ecosystem | ❌ | ✅ (any no_std-compatible crate) |
