use bunnydb_http::{BunnyDbClient, Statement, StatementOutcome, Value};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("BUNNYDB_PIPELINE_URL")?;
    let token = std::env::var("BUNNYDB_TOKEN")?;

    let db = BunnyDbClient::new_bearer(url, token);

    let outcomes = db
        .batch([
            Statement::execute(
                "CREATE TABLE IF NOT EXISTS batch_users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
                (),
            ),
            Statement::execute(
                "INSERT INTO batch_users (name) VALUES (?)",
                [Value::text("Alice")],
            ),
            Statement::query("SELECT id, name FROM batch_users", ()),
        ])
        .await?;

    for outcome in outcomes {
        match outcome {
            StatementOutcome::Exec(exec) => println!(
                "exec: affected={}, last_insert_rowid={:?}",
                exec.affected_row_count, exec.last_insert_rowid
            ),
            StatementOutcome::Query(query) => {
                println!("query: {} row(s)", query.rows.len());
            }
            StatementOutcome::SqlError {
                request_index,
                message,
                ..
            } => {
                eprintln!("sql error at index {request_index}: {message}");
            }
        }
    }

    Ok(())
}
