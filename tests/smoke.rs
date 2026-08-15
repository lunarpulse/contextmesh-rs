//! Embedded Turso baseline smoke test.

use std::error::Error;

#[tokio::test(flavor = "current_thread")]
async fn in_memory_turso_write_read_round_trip() -> Result<(), Box<dyn Error>> {
    let database = turso::Builder::new_local(":memory:").build().await?;
    let connection = database.connect()?;

    connection
        .execute(
            "CREATE TABLE baseline_smoke (id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            (),
        )
        .await?;
    connection
        .execute(
            "INSERT INTO baseline_smoke (id, value) VALUES (1, 'local-turso-ok')",
            (),
        )
        .await?;

    let mut rows = connection
        .query("SELECT value FROM baseline_smoke WHERE id = 1", ())
        .await?;
    let row = rows.next().await?.ok_or("smoke row was not returned")?;
    let value: String = row.get(0)?;

    assert_eq!(value, "local-turso-ok");
    assert!(rows.next().await?.is_none());
    Ok(())
}
