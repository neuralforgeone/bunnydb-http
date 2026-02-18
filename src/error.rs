#[derive(Debug, thiserror::Error)]
pub enum BunnyDbError {
    #[error("transport error: {0}")]
    Transport(reqwest::Error),
    #[error("http error {status}: {body}")]
    Http { status: u16, body: String },
    #[error("pipeline error at request {request_index}: {message}")]
    Pipeline {
        request_index: usize,
        message: String,
        code: Option<String>,
    },
    #[error("decode error: {0}")]
    Decode(String),
}
