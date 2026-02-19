/// Configures HTTP timeout and retry behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientOptions {
    /// Per-request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Maximum number of retries after the initial attempt.
    pub max_retries: usize,
    /// Base retry backoff in milliseconds (exponential strategy).
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
