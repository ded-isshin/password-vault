use std::{
    env,
    io::{Error, ErrorKind},
};

use password_vault_api::{ApiConfig, build_app, init_tracing, run_database_migrations};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    if let Some(command) = env::args().nth(1) {
        return match command.as_str() {
            "migrate" => run_migrations_and_exit().await,
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unknown command '{command}'; expected no argument or 'migrate'"),
            )
            .into()),
        };
    }

    let config = ApiConfig::from_env()?;
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    let bind_addr = config.bind_addr;
    let database_configured = config.database_url_present();
    let app = build_app(config).await?;

    tracing::info!(
        bind_addr = %bind_addr,
        database_configured,
        service = "password-vault-api",
        "starting API service"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn run_migrations_and_exit() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("PV_DATABASE_URL")
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "PV_DATABASE_URL is required"))?;
    let database_url = database_url.trim();
    if database_url.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "PV_DATABASE_URL is required").into());
    }

    tracing::info!(
        service = "password-vault-api",
        "running database migrations"
    );
    run_database_migrations(database_url).await?;
    tracing::info!(
        service = "password-vault-api",
        "database migrations completed"
    );

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
