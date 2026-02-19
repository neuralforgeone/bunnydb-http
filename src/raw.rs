//! Experimental raw response passthrough types.
//!
//! Enabled with the `raw-mode` feature.

/// Wrapper around raw JSON pipeline response payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RawPipelineResponse(pub serde_json::Value);
