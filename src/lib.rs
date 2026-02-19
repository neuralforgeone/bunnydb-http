//! Async HTTP client for Bunny.net Database SQL pipeline API.
//!
//! This crate wraps the `/v2/pipeline` endpoint with ergonomic methods:
//!
//! - [`BunnyDbClient::query`]
//! - [`BunnyDbClient::execute`]
//! - [`BunnyDbClient::batch`]
//!
//! ## Client Construction
//!
//! Choose the constructor that fits your deployment:
//!
//! | Constructor | When to use |
//! |---|---|
//! | [`BunnyDbClient::from_env`] | 12-factor / container: `BUNNYDB_PIPELINE_URL` + `BUNNYDB_TOKEN` |
//! | [`BunnyDbClient::from_env_db_id`] | Edge scripts / containers: `BUNNYDB_ID` + `BUNNYDB_TOKEN` |
//! | [`BunnyDbClient::from_db_id`] | Hardcoded DB ID, token from config |
//! | [`BunnyDbClient::new_bearer`] | Full URL + bearer token |
//! | [`BunnyDbClient::new_raw_auth`] | Full URL + custom auth header |
//!
//! # Quick Start — environment variables
//!
//! ```no_run
//! use bunnydb_http::BunnyDbClient;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Reads BUNNYDB_PIPELINE_URL and BUNNYDB_TOKEN automatically
//! let db = BunnyDbClient::from_env().expect("missing BUNNYDB_* env vars");
//!
//! db.execute(
//!     "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
//!     (),
//! ).await?;
//!
//! let result = db.query(
//!     "SELECT id, name FROM users WHERE name = :name",
//!     bunnydb_http::Params::named([("name", bunnydb_http::Value::text("Kit"))]),
//! ).await?;
//!
//! println!("rows={}", result.rows.len());
//! # Ok(())
//! # }
//! ```

mod client;
mod decode;
mod error;
mod options;
mod params;
mod types;
mod value;
mod wire;

#[cfg(feature = "baton-experimental")]
pub mod baton;
#[cfg(feature = "raw-mode")]
pub mod raw;
#[cfg(feature = "row-map")]
pub mod row_map;

pub use client::{db_id_to_pipeline_url, BunnyDbClient};
pub use error::BunnyDbError;
pub use options::ClientOptions;
pub use params::{Params, Statement};
pub use types::{Col, ExecResult, QueryResult, StatementOutcome};
pub use value::Value;

/// Crate-wide result type.
pub type Result<T> = std::result::Result<T, BunnyDbError>;
