use std::{
    env,
    io::{Error, ErrorKind},
};

use tokio::sync::watch;

use password_vault_api::{ApiConfig, build_api_and_metrics, init_tracing, run_database_migrations};

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
    let metrics_listener = tokio::net::TcpListener::bind(config.metrics_bind_addr).await?;
    let bind_addr = config.bind_addr;
    let metrics_bind_addr = config.metrics_bind_addr;
    let database_configured = config.database_url_present();
    let (app, metrics_app) = build_api_and_metrics(config).await?;

    tracing::info!(
        bind_addr = %bind_addr,
        metrics_bind_addr = %metrics_bind_addr,
        database_configured,
        service = "password-vault-api",
        "starting API service"
    );

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(true);
    });

    let api_server =
        axum::serve(listener, app).with_graceful_shutdown(wait_for_shutdown(shutdown_rx.clone()));
    let metrics_server = axum::serve(metrics_listener, metrics_app)
        .with_graceful_shutdown(wait_for_shutdown(shutdown_rx));

    tokio::try_join!(api_server, metrics_server)?;

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

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    while !*shutdown_rx.borrow() {
        if shutdown_rx.changed().await.is_err() {
            break;
        }
    }
}
