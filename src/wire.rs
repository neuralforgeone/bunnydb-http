use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct PipelineRequest {
    pub requests: Vec<Request>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Execute { stmt: ExecuteStatement },
    Close {},
}

#[derive(Debug, Serialize)]
pub struct ExecuteStatement {
    pub sql: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named_args: Option<Vec<NamedArg>>,
    pub want_rows: bool,
}

#[derive(Debug, Serialize)]
pub struct NamedArg {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Value {
    Null {},
    Integer { value: String },
    Float { value: String },
    Text { value: String },
    Blob { base64: String },
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PipelineResponse {
    #[serde(default)]
    pub baton: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    pub results: Vec<PipelineResult>,
}

#[derive(Debug, Deserialize)]
pub struct PipelineResult {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub response: Option<ResponseEnvelope>,
    #[serde(default)]
    pub error: Option<PipelineError>,
}

#[derive(Debug, Deserialize)]
pub struct PipelineError {
    pub message: String,
    #[serde(default)]
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseEnvelope {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub result: Option<ExecuteResult>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ExecuteResult {
    #[serde(default)]
    pub cols: Vec<Col>,
    #[serde(default)]
    pub rows: Vec<Vec<Value>>,
    #[serde(default)]
    pub affected_row_count: u64,
    #[serde(default)]
    pub last_insert_rowid: Option<String>,
    #[serde(default)]
    pub replication_index: Option<String>,
    #[serde(default)]
    pub rows_read: Option<u64>,
    #[serde(default)]
    pub rows_written: Option<u64>,
    #[serde(default)]
    pub query_duration_ms: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Col {
    pub name: String,
    #[serde(default)]
    pub decltype: Option<String>,
}
