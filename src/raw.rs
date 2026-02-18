//! Experimental raw response passthrough types.
//!
//! Enabled with the `raw-mode` feature.

#[derive(Clone, Debug, PartialEq)]
pub struct RawPipelineResponse(pub serde_json::Value);
