//! Async HTTP client for Bunny.net Database SQL pipeline API.
//!
//! This crate wraps the `/v2/pipeline` endpoint with ergonomic methods:
//!
//! - [`BunnyDbClient::query`]
//! - [`BunnyDbClient::execute`]
//! - [`BunnyDbClient::batch`]
//!
//! # Quick Start
//!
//! ```no_run
//! use bunnydb_http::{BunnyDbClient, Params, Value};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let pipeline_url = std::env::var("BUNNYDB_PIPELINE_URL")?;
//! let token = std::env::var("BUNNYDB_TOKEN")?;
//! let db = BunnyDbClient::new_bearer(pipeline_url, token);
//!
//! db.execute(
//!     "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
//!     (),
//! ).await?;
//!
//! let result = db.query(
//!     "SELECT id, name FROM users WHERE name = :name",
//!     Params::named([("name", Value::text("Kit"))]),
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

pub use client::BunnyDbClient;
pub use error::BunnyDbError;
pub use options::ClientOptions;
pub use params::{Params, Statement};
pub use types::{Col, ExecResult, QueryResult, StatementOutcome};
pub use value::Value;

/// Crate-wide result type.
pub type Result<T> = std::result::Result<T, BunnyDbError>;
