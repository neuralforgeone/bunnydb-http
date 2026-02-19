/**
 * Bunny Edge Script — Rust/WASM BunnyDB Handler
 *
 * This TypeScript file is the thin host that:
 *   1. Loads the compiled Rust WASM module from Bunny Storage / CDN
 *   2. Instantiates the `BunnyEdgeHandler` (defined in Rust src/lib.rs)
 *   3. Routes incoming HTTP requests to Rust handler methods
 *
 * Deploy steps:
 *   1. Build WASM:  wasm-pack build --target bundler --release
 *   2. Upload `pkg/bunnydb_edge_handler_bg.wasm` to Bunny Storage
 *   3. Set env vars: DB_URL (or DB_ID) + DB_TOKEN in your Edge Script settings
 *   4. Deploy this file as your Bunny Edge Script
 *
 * The Rust code (src/lib.rs) handles all BunnyDB queries — this file only
 * routes requests and returns HTTP responses.
 */

import * as BunnySDK from "https://esm.sh/@bunny.net/edgescript-sdk@0.12.0";
import process from "node:process";
import init, { BunnyEdgeHandler } from "./pkg/bunnydb_edge_handler.js";

// ── Bootstrap ──────────────────────────────────────────────────────────────

// Load and instantiate the WASM module. This runs once at cold start.
// The .wasm file is fetched from Bunny Storage for minimum latency.
await init(fetch(process.env.WASM_URL ?? "https://your-zone.b-cdn.net/bunnydb_edge_handler_bg.wasm"));

// Create the Rust BunnyDB handler.
// Credentials are injected by Bunny via environment variables (Access page → Generate Token).
const db = process.env.DB_ID
    ? BunnyEdgeHandler.from_db_id(process.env.DB_ID, process.env.DB_TOKEN!)
    : new BunnyEdgeHandler(process.env.DB_URL!, process.env.DB_TOKEN!);

// ── Router ─────────────────────────────────────────────────────────────────

BunnySDK.net.http.serve(async (req: Request): Promise<Response> => {
    const url = new URL(req.url);
    const method = req.method.toUpperCase();

    // GET /users → query all users (handled in Rust via BunnyDB HTTP)
    if (method === "GET" && url.pathname === "/users") {
        try {
            const json = await db.query_json("SELECT id, name, email FROM users ORDER BY id DESC LIMIT 50");
            return new Response(json, {
                status: 200,
                headers: { "Content-Type": "application/json" },
            });
        } catch (err) {
            return errorResponse(500, String(err));
        }
    }

    // POST /users  body: { "name": "...", "email": "..." }
    if (method === "POST" && url.pathname === "/users") {
        try {
            const body = await req.json() as { name?: string; email?: string };
            if (!body.name || !body.email) {
                return errorResponse(400, "name and email are required");
            }

            const result = await db.insert_one(
                "INSERT INTO users (name, email) VALUES (?, ?)",
                JSON.stringify([body.name, body.email]),
            );

            return new Response(result, {
                status: 201,
                headers: { "Content-Type": "application/json" },
            });
        } catch (err) {
            return errorResponse(500, String(err));
        }
    }

    // GET /health → simple liveness check (no DB call)
    if (method === "GET" && url.pathname === "/health") {
        return new Response(JSON.stringify({ status: "ok", runtime: "rust+wasm" }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
        });
    }

    return errorResponse(404, `not found: ${url.pathname}`);
});

// ── Helpers ────────────────────────────────────────────────────────────────

function errorResponse(status: number, message: string): Response {
    return new Response(JSON.stringify({ error: message }), {
        status,
        headers: { "Content-Type": "application/json" },
    });
}
