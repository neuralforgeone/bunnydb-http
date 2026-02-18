#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientOptions {
    pub timeout_ms: u64,
    pub max_retries: usize,
    pub retry_backoff_ms: u64,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 10_000,
            max_retries: 0,
            retry_backoff_ms: 250,
        }
    }
}
