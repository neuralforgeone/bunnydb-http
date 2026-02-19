//! Experimental baton/session types.
//!
//! Enabled with the `baton-experimental` feature.

/// Session baton value returned by Bunny.net pipeline API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Baton(pub String);
