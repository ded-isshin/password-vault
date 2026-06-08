use std::time::Duration;

use sqlx::{PgPool, postgres::PgPoolOptions};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
pub const DATABASE_MAX_CONNECTIONS: u32 = 5;
const DATABASE_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    pool_options().connect(database_url).await
}

pub fn connect_lazy(database_url: &str) -> Result<PgPool, sqlx::Error> {
    pool_options().connect_lazy(database_url)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}

pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await.map(|_| ())
}

fn pool_options() -> PgPoolOptions {
    PgPoolOptions::new()
        .max_connections(DATABASE_MAX_CONNECTIONS)
        .acquire_timeout(DATABASE_ACQUIRE_TIMEOUT)
}
