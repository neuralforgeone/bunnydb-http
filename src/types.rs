use crate::Value;

/// Column metadata returned by query responses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Col {
    /// Column name.
    pub name: String,
    /// Declared SQL type, if present.
    pub decltype: Option<String>,
}

/// Query response shape.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult {
    /// Column metadata.
    pub cols: Vec<Col>,
    /// Decoded row values.
    pub rows: Vec<Vec<Value>>,
    /// Optional replication index returned by API.
    pub replication_index: Option<String>,
    /// Optional number of rows read by query.
    pub rows_read: Option<u64>,
    /// Optional number of rows written by query.
    pub rows_written: Option<u64>,
    /// Optional execution duration in milliseconds.
    pub query_duration_ms: Option<f64>,
}

/// Execute response shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecResult {
    /// Number of affected rows.
    pub affected_row_count: u64,
    /// Last inserted row id if returned by engine.
    pub last_insert_rowid: Option<i64>,
    /// Optional replication index returned by API.
    pub replication_index: Option<String>,
    /// Optional number of rows read during execution.
    pub rows_read: Option<u64>,
    /// Optional number of rows written during execution.
    pub rows_written: Option<u64>,
}

/// Batch outcome per statement.
#[derive(Clone, Debug, PartialEq)]
pub enum StatementOutcome {
    /// Successful query statement.
    Query(QueryResult),
    /// Successful execute statement.
    Exec(ExecResult),
    /// Statement-level SQL error from pipeline response.
    SqlError {
        /// Index of statement in request batch.
        request_index: usize,
        /// SQL error message.
        message: String,
        /// Optional SQL error code.
        code: Option<String>,
    },
}
