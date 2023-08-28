use diesel::{Connection, PgConnection};
use diesel_migrations::{
    embed_migrations, EmbeddedMigrations, HarnessWithOutput, MigrationHarness,
};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

pub fn run(conn: &mut PgConnection) -> anyhow::Result<()> {
    tracing::info!("Database migration starting");

    // Transaction such that no migrations are applied if one doesn't succeed
    conn.transaction(|conn| {
        let mut harness = HarnessWithOutput::write_to_stdout(conn);
        harness
            .run_pending_migrations(MIGRATIONS)
            .expect("database migration failed");

        anyhow::Ok(())
    })?;

    tracing::info!("Database migration succeeded");

    Ok(())
}
