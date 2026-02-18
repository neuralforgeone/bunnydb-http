use crate::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Col {
    pub name: String,
    pub decltype: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult {
    pub cols: Vec<Col>,
    pub rows: Vec<Vec<Value>>,
    pub replication_index: Option<String>,
    pub rows_read: Option<u64>,
    pub rows_written: Option<u64>,
    pub query_duration_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecResult {
    pub affected_row_count: u64,
    pub last_insert_rowid: Option<i64>,
    pub replication_index: Option<String>,
    pub rows_read: Option<u64>,
    pub rows_written: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatementOutcome {
    Query(QueryResult),
    Exec(ExecResult),
    SqlError {
        request_index: usize,
        message: String,
        code: Option<String>,
    },
}
