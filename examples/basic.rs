use bunnydb_http::{BunnyDbClient, Params, Value};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("BUNNYDB_PIPELINE_URL")?;
    let token = std::env::var("BUNNYDB_TOKEN")?;

    let db = BunnyDbClient::new_bearer(url, token);

    db.execute(
        "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        (),
    )
    .await?;

    db.execute("INSERT INTO users (name) VALUES (?)", [Value::text("Kit")])
        .await?;

    let result = db
        .query(
            "SELECT id, name FROM users WHERE name = :name",
            Params::named([("name", Value::text("Kit"))]),
        )
        .await?;

    for row in result.rows {
        println!("{row:?}");
    }

    Ok(())
}
