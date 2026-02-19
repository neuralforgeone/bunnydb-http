/// Error type returned by this crate.
#[derive(Debug, thiserror::Error)]
pub enum BunnyDbError {
    /// Network or request execution error from `reqwest`.
    #[error("transport error: {0}")]
    Transport(reqwest::Error),
    /// Non-success HTTP status code with raw response body.
    #[error("http error {status}: {body}")]
    Http { status: u16, body: String },
    /// SQL/pipeline error returned by Bunny.net API.
    #[error("pipeline error at request {request_index}: {message}")]
    Pipeline {
        /// Index of the failing request in the pipeline payload.
        request_index: usize,
        /// Error message text from upstream API.
        message: String,
        /// Optional engine-specific error code.
        code: Option<String>,
    },
    /// Response decoding or protocol-shape validation error.
    #[error("decode error: {0}")]
    Decode(String),
}
