//! `bunnydb-http` is an async HTTP client for Bunny.net Database SQL API.
//!
//! The crate wraps the `/v2/pipeline` endpoint with ergonomic methods:
//! - [`BunnyDbClient::query`]
//! - [`BunnyDbClient::execute`]
//! - [`BunnyDbClient::batch`]

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

pub type Result<T> = std::result::Result<T, BunnyDbError>;
