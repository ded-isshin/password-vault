use std::time::{Duration, Instant};

use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::telemetry;

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
    const OPERATION: &str = "readyz_ping";

    let acquire_started = Instant::now();
    let mut connection = match pool.acquire().await {
        Ok(connection) => {
            telemetry::db_pool_wait_duration(OPERATION, "success", acquire_started.elapsed());
            connection
        }
        Err(error) => {
            telemetry::db_pool_wait_duration(OPERATION, "error", acquire_started.elapsed());
            telemetry::db_error(OPERATION, sqlx_error_class(&error));
            return Err(error);
        }
    };

    let query_started = Instant::now();
    match sqlx::query("SELECT 1").execute(&mut *connection).await {
        Ok(_) => {
            telemetry::db_query_duration(OPERATION, "success", query_started.elapsed());
            Ok(())
        }
        Err(error) => {
            telemetry::db_query_duration(OPERATION, "error", query_started.elapsed());
            telemetry::db_error(OPERATION, sqlx_error_class(&error));
            Err(error)
        }
    }
}

fn pool_options() -> PgPoolOptions {
    PgPoolOptions::new()
        .max_connections(DATABASE_MAX_CONNECTIONS)
        .acquire_timeout(DATABASE_ACQUIRE_TIMEOUT)
}

fn sqlx_error_class(error: &sqlx::Error) -> &'static str {
    match error {
        sqlx::Error::PoolTimedOut => "pool_timeout",
        sqlx::Error::PoolClosed => "pool_closed",
        sqlx::Error::Database(_) => "database",
        sqlx::Error::Io(_) => "io",
        sqlx::Error::Tls(_) => "tls",
        sqlx::Error::Configuration(_) => "configuration",
        sqlx::Error::Protocol(_) => "protocol",
        sqlx::Error::RowNotFound => "row_not_found",
        sqlx::Error::TypeNotFound { .. } => "type_not_found",
        sqlx::Error::ColumnIndexOutOfBounds { .. }
        | sqlx::Error::ColumnNotFound(_)
        | sqlx::Error::ColumnDecode { .. }
        | sqlx::Error::Decode(_) => "decode",
        sqlx::Error::AnyDriverError(_) => "driver",
        sqlx::Error::Migrate(_) => "migration",
        _ => "other",
    }
}
