use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};

use bunnydb_http::{
    BunnyDbClient, ExecResult, Params, QueryResult, Statement, StatementOutcome, Value,
};
use eframe::egui::{self, Color32, RichText, TextEdit};
use serde::Deserialize;
use serde_json::Value as JsonValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthMode {
    Bearer,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OperationMode {
    Query,
    Execute,
    Batch,
}

#[derive(Debug)]
enum UiResponse {
    Query(Result<QueryResult, String>),
    Execute(Result<ExecResult, String>),
    Batch(Result<Vec<StatementOutcome>, String>),
}

#[derive(Debug)]
enum LastResult {
    Query(QueryResult),
    Execute(ExecResult),
    Batch(Vec<StatementOutcome>),
}

#[derive(Debug, Deserialize)]
struct BatchInputStatement {
    kind: String,
    sql: String,
    #[serde(default)]
    params: Option<JsonValue>,
}

struct BunnyGuiApp {
    auth_mode: AuthMode,
    mode: OperationMode,
    pipeline_url: String,
    token_or_authorization: String,
    query_sql: String,
    query_params_json: String,
    execute_sql: String,
    execute_params_json: String,
    batch_json: String,
    status: String,
    in_flight: bool,
    rx: Option<Receiver<UiResponse>>,
    last_result: Option<LastResult>,
    last_error: Option<String>,
}

impl Default for BunnyGuiApp {
    fn default() -> Self {
        Self {
            auth_mode: AuthMode::Bearer,
            mode: OperationMode::Query,
            pipeline_url: String::new(),
            token_or_authorization: String::new(),
            query_sql: "SELECT 1 AS ok".to_owned(),
            query_params_json: "[]".to_owned(),
            execute_sql: "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_owned(),
            execute_params_json: "[]".to_owned(),
            batch_json: r#"[
  { "kind": "execute", "sql": "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)" },
  { "kind": "execute", "sql": "INSERT INTO users (name) VALUES (?)", "params": ["Kit"] },
  { "kind": "query", "sql": "SELECT id, name FROM users", "params": [] }
]"#
            .to_owned(),
            status: "Ready".to_owned(),
            in_flight: false,
            rx: None,
            last_result: None,
            last_error: None,
        }
    }
}

impl eframe::App for BunnyGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_response();

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.heading("BunnyDB HTTP GUI Client");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Pipeline URL");
                ui.add(
                    TextEdit::singleline(&mut self.pipeline_url)
                        .hint_text("https://<db-id>.lite.bunnydb.net/v2/pipeline")
                        .desired_width(600.0),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Auth Mode");
                ui.selectable_value(&mut self.auth_mode, AuthMode::Bearer, "Bearer Token");
                ui.selectable_value(&mut self.auth_mode, AuthMode::Raw, "Raw Authorization");
            });

            ui.horizontal(|ui| {
                let label = match self.auth_mode {
                    AuthMode::Bearer => "Token",
                    AuthMode::Raw => "Authorization",
                };
                ui.label(label);
                ui.add(
                    TextEdit::singleline(&mut self.token_or_authorization)
                        .password(true)
                        .desired_width(450.0),
                );
            });

            ui.horizontal(|ui| {
                ui.label("Mode");
                ui.selectable_value(&mut self.mode, OperationMode::Query, "Query");
                ui.selectable_value(&mut self.mode, OperationMode::Execute, "Execute");
                ui.selectable_value(&mut self.mode, OperationMode::Batch, "Batch");
            });

            ui.horizontal(|ui| {
                let status_color = if self.last_error.is_some() {
                    Color32::from_rgb(215, 40, 40)
                } else {
                    Color32::from_rgb(35, 120, 35)
                };
                ui.label(RichText::new(format!("Status: {}", self.status)).color(status_color));

                if self.in_flight {
                    ui.spinner();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.mode {
                OperationMode::Query => self.render_query_ui(ui),
                OperationMode::Execute => self.render_execute_ui(ui),
                OperationMode::Batch => self.render_batch_ui(ui),
            }

            ui.separator();
            self.render_results_ui(ui);
        });

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl BunnyGuiApp {
    fn render_query_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Query");
        ui.label("SQL");
        ui.add(
            TextEdit::multiline(&mut self.query_sql)
                .desired_rows(6)
                .code_editor()
                .desired_width(f32::INFINITY),
        );
        ui.label("Params JSON (`[]` for positional, `{}` for named)");
        ui.add(
            TextEdit::multiline(&mut self.query_params_json)
                .desired_rows(4)
                .code_editor()
                .desired_width(f32::INFINITY),
        );

        if ui
            .add_enabled(!self.in_flight, egui::Button::new("Run Query"))
            .clicked()
        {
            self.run_query_async();
        }
    }

    fn render_execute_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Execute");
        ui.label("SQL");
        ui.add(
            TextEdit::multiline(&mut self.execute_sql)
                .desired_rows(6)
                .code_editor()
                .desired_width(f32::INFINITY),
        );
        ui.label("Params JSON (`[]` for positional, `{}` for named)");
        ui.add(
            TextEdit::multiline(&mut self.execute_params_json)
                .desired_rows(4)
                .code_editor()
                .desired_width(f32::INFINITY),
        );

        if ui
            .add_enabled(!self.in_flight, egui::Button::new("Run Execute"))
            .clicked()
        {
            self.run_execute_async();
        }
    }

    fn render_batch_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Batch");
        ui.label("Batch JSON (array of `{ kind, sql, params? }`; `kind` is `query` or `execute`)");
        ui.add(
            TextEdit::multiline(&mut self.batch_json)
                .desired_rows(12)
                .code_editor()
                .desired_width(f32::INFINITY),
        );

        if ui
            .add_enabled(!self.in_flight, egui::Button::new("Run Batch"))
            .clicked()
        {
            self.run_batch_async();
        }
    }

    fn render_results_ui(&self, ui: &mut egui::Ui) {
        ui.heading("Results");

        if let Some(error) = &self.last_error {
            ui.colored_label(Color32::from_rgb(215, 40, 40), error);
            return;
        }

        match &self.last_result {
            Some(LastResult::Query(result)) => render_query_result(ui, result),
            Some(LastResult::Execute(result)) => render_exec_result(ui, result),
            Some(LastResult::Batch(outcomes)) => render_batch_result(ui, outcomes),
            None => {
                ui.label("No result yet.");
            }
        }
    }

    fn run_query_async(&mut self) {
        let pipeline_url = self.pipeline_url.clone();
        let auth = self.token_or_authorization.clone();
        let mode = self.auth_mode;
        let sql = self.query_sql.clone();
        let params_json = self.query_params_json.clone();

        self.status = "Running query...".to_owned();
        self.in_flight = true;
        self.last_error = None;
        self.last_result = None;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let response = run_query_request(pipeline_url, auth, mode, sql, params_json);
            let _ = tx.send(UiResponse::Query(response));
        });
    }

    fn run_execute_async(&mut self) {
        let pipeline_url = self.pipeline_url.clone();
        let auth = self.token_or_authorization.clone();
        let mode = self.auth_mode;
        let sql = self.execute_sql.clone();
        let params_json = self.execute_params_json.clone();

        self.status = "Running execute...".to_owned();
        self.in_flight = true;
        self.last_error = None;
        self.last_result = None;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let response = run_execute_request(pipeline_url, auth, mode, sql, params_json);
            let _ = tx.send(UiResponse::Execute(response));
        });
    }

    fn run_batch_async(&mut self) {
        let pipeline_url = self.pipeline_url.clone();
        let auth = self.token_or_authorization.clone();
        let mode = self.auth_mode;
        let batch_json = self.batch_json.clone();

        self.status = "Running batch...".to_owned();
        self.in_flight = true;
        self.last_error = None;
        self.last_result = None;

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        std::thread::spawn(move || {
            let response = run_batch_request(pipeline_url, auth, mode, batch_json);
            let _ = tx.send(UiResponse::Batch(response));
        });
    }

    fn poll_response(&mut self) {
        let Some(rx) = &self.rx else {
            return;
        };

        match rx.try_recv() {
            Ok(message) => {
                self.in_flight = false;
                self.rx = None;
                match message {
                    UiResponse::Query(result) => match result {
                        Ok(value) => {
                            self.status = format!("Query OK ({} rows)", value.rows.len());
                            self.last_result = Some(LastResult::Query(value));
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.status = "Query failed".to_owned();
                            self.last_error = Some(err);
                            self.last_result = None;
                        }
                    },
                    UiResponse::Execute(result) => match result {
                        Ok(value) => {
                            self.status =
                                format!("Execute OK (affected {})", value.affected_row_count);
                            self.last_result = Some(LastResult::Execute(value));
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.status = "Execute failed".to_owned();
                            self.last_error = Some(err);
                            self.last_result = None;
                        }
                    },
                    UiResponse::Batch(result) => match result {
                        Ok(value) => {
                            self.status = format!("Batch OK ({} outcomes)", value.len());
                            self.last_result = Some(LastResult::Batch(value));
                            self.last_error = None;
                        }
                        Err(err) => {
                            self.status = "Batch failed".to_owned();
                            self.last_error = Some(err);
                            self.last_result = None;
                        }
                    },
                }
            }
            Err(TryRecvError::Disconnected) => {
                self.in_flight = false;
                self.rx = None;
                self.status = "Worker disconnected".to_owned();
                self.last_error = Some("Background worker disconnected unexpectedly.".to_owned());
            }
            Err(TryRecvError::Empty) => {}
        }
    }
}

fn build_client(pipeline_url: String, auth: String, mode: AuthMode) -> BunnyDbClient {
    match mode {
        AuthMode::Bearer => BunnyDbClient::new_bearer(pipeline_url, auth),
        AuthMode::Raw => BunnyDbClient::new_raw_auth(pipeline_url, auth),
    }
}

fn run_query_request(
    pipeline_url: String,
    auth: String,
    mode: AuthMode,
    sql: String,
    params_json: String,
) -> Result<QueryResult, String> {
    validate_connection_fields(&pipeline_url, &auth)?;
    let params = parse_params_json(&params_json)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("runtime init failed: {err}"))?;
    runtime.block_on(async move {
        let client = build_client(pipeline_url, auth, mode);
        client
            .query(&sql, params)
            .await
            .map_err(|err| format!("query error: {err}"))
    })
}

fn run_execute_request(
    pipeline_url: String,
    auth: String,
    mode: AuthMode,
    sql: String,
    params_json: String,
) -> Result<ExecResult, String> {
    validate_connection_fields(&pipeline_url, &auth)?;
    let params = parse_params_json(&params_json)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("runtime init failed: {err}"))?;
    runtime.block_on(async move {
        let client = build_client(pipeline_url, auth, mode);
        client
            .execute(&sql, params)
            .await
            .map_err(|err| format!("execute error: {err}"))
    })
}

fn run_batch_request(
    pipeline_url: String,
    auth: String,
    mode: AuthMode,
    batch_json: String,
) -> Result<Vec<StatementOutcome>, String> {
    validate_connection_fields(&pipeline_url, &auth)?;
    let statements = parse_batch_json(&batch_json)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("runtime init failed: {err}"))?;
    runtime.block_on(async move {
        let client = build_client(pipeline_url, auth, mode);
        client
            .batch(statements)
            .await
            .map_err(|err| format!("batch error: {err}"))
    })
}

fn validate_connection_fields(pipeline_url: &str, auth: &str) -> Result<(), String> {
    if pipeline_url.trim().is_empty() {
        return Err("pipeline URL is required".to_owned());
    }
    if auth.trim().is_empty() {
        return Err("token/authorization is required".to_owned());
    }
    Ok(())
}

fn parse_batch_json(input: &str) -> Result<Vec<Statement>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("batch JSON cannot be empty".to_owned());
    }

    let parsed: Vec<BatchInputStatement> =
        serde_json::from_str(trimmed).map_err(|err| format!("invalid batch JSON: {err}"))?;

    let mut out = Vec::with_capacity(parsed.len());
    for (index, entry) in parsed.into_iter().enumerate() {
        if entry.sql.trim().is_empty() {
            return Err(format!("batch[{index}] has empty SQL"));
        }

        let params = match entry.params {
            Some(value) => parse_params_value(value)?,
            None => Params::default(),
        };

        if entry.kind.eq_ignore_ascii_case("query") {
            out.push(Statement::query(entry.sql, params));
        } else if entry.kind.eq_ignore_ascii_case("execute") {
            out.push(Statement::execute(entry.sql, params));
        } else {
            return Err(format!(
                "batch[{index}] invalid kind '{}': expected 'query' or 'execute'",
                entry.kind
            ));
        }
    }

    Ok(out)
}

fn parse_params_json(input: &str) -> Result<Params, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Params::default());
    }
    let parsed = serde_json::from_str::<JsonValue>(trimmed)
        .map_err(|err| format!("invalid params JSON: {err}"))?;
    parse_params_value(parsed)
}

fn parse_params_value(value: JsonValue) -> Result<Params, String> {
    match value {
        JsonValue::Array(items) => {
            let values = items
                .into_iter()
                .map(parse_value_json)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Params::positional(values))
        }
        JsonValue::Object(map) => {
            let values = map
                .into_iter()
                .map(|(key, value)| parse_value_json(value).map(|parsed| (key, parsed)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Params::named(values))
        }
        JsonValue::Null => Ok(Params::default()),
        _ => Err("params JSON must be either array, object, or null".to_owned()),
    }
}

fn parse_value_json(value: JsonValue) -> Result<Value, String> {
    match value {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(flag) => Ok(Value::integer(i64::from(flag))),
        JsonValue::Number(number) => {
            if let Some(i) = number.as_i64() {
                return Ok(Value::integer(i));
            }
            if let Some(f) = number.as_f64() {
                if !f.is_finite() {
                    return Err("non-finite float is not supported".to_owned());
                }
                return Ok(Value::float(f));
            }
            Err(format!("unsupported number '{number}'"))
        }
        JsonValue::String(text) => Ok(Value::text(text)),
        JsonValue::Array(_) => {
            Err("nested arrays are not supported in parameter values".to_owned())
        }
        JsonValue::Object(mut map) => {
            if map.len() == 1 {
                if let Some(JsonValue::String(blob)) = map.remove("blob_base64") {
                    return Ok(Value::blob_base64(blob));
                }
            }
            Err("object parameter values must be {\"blob_base64\": \"...\"}".to_owned())
        }
    }
}

fn render_query_result(ui: &mut egui::Ui, result: &QueryResult) {
    ui.label(format!("Rows: {}", result.rows.len()));
    ui.label(format!("Replication index: {:?}", result.replication_index));
    ui.label(format!("Rows read: {:?}", result.rows_read));
    ui.label(format!("Rows written: {:?}", result.rows_written));
    ui.label(format!("Duration (ms): {:?}", result.query_duration_ms));
    ui.separator();

    if result.cols.is_empty() {
        ui.label("No columns returned.");
        return;
    }

    egui::ScrollArea::both().max_height(360.0).show(ui, |ui| {
        egui::Grid::new("query_result_grid")
            .striped(true)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for col in &result.cols {
                    ui.label(RichText::new(&col.name).strong());
                }
                ui.end_row();

                for row in &result.rows {
                    for value in row {
                        ui.monospace(display_value(value));
                    }
                    ui.end_row();
                }
            });
    });
}

fn render_exec_result(ui: &mut egui::Ui, result: &ExecResult) {
    ui.label(format!("Affected rows: {}", result.affected_row_count));
    ui.label(format!("Last insert rowid: {:?}", result.last_insert_rowid));
    ui.label(format!("Replication index: {:?}", result.replication_index));
    ui.label(format!("Rows read: {:?}", result.rows_read));
    ui.label(format!("Rows written: {:?}", result.rows_written));
}

fn render_batch_result(ui: &mut egui::Ui, outcomes: &[StatementOutcome]) {
    ui.label(format!("Outcomes: {}", outcomes.len()));
    ui.separator();

    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            for (index, outcome) in outcomes.iter().enumerate() {
                match outcome {
                    StatementOutcome::Exec(exec) => {
                        ui.label(format!(
                            "[{index}] EXEC ok: affected={}, last_insert_rowid={:?}",
                            exec.affected_row_count, exec.last_insert_rowid
                        ));
                    }
                    StatementOutcome::Query(query) => {
                        ui.label(format!(
                            "[{index}] QUERY ok: rows={}, duration_ms={:?}",
                            query.rows.len(),
                            query.query_duration_ms
                        ));
                    }
                    StatementOutcome::SqlError {
                        request_index,
                        message,
                        code,
                    } => {
                        ui.colored_label(
                            Color32::from_rgb(215, 40, 40),
                            format!(
                                "[{index}] SQL error at request {}: {} ({:?})",
                                request_index, message, code
                            ),
                        );
                    }
                }
            }
        });
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Integer(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::Text(v) => v.clone(),
        Value::BlobBase64(v) => format!("<blob:{} chars>", v.len()),
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "BunnyDB HTTP GUI",
        options,
        Box::new(|_cc| Box::new(BunnyGuiApp::default())),
    )
}
