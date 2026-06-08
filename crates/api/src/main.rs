use std::{
    env,
    io::{Error, ErrorKind},
};

use tokio::sync::watch;

use password_vault_api::{
    ApiConfig, build_api_and_metrics, db, init_tracing,
    maintenance::{SyntheticCleanupOptions, cleanup_synthetic_accounts},
    run_database_migrations,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let mut args = env::args().skip(1);
    if let Some(command) = args.next() {
        return match command.as_str() {
            "migrate" => run_migrations_and_exit().await,
            "cleanup-synthetic" => run_synthetic_cleanup_and_exit(args.collect()).await,
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "unknown command '{command}'; expected no argument, 'migrate', or 'cleanup-synthetic'"
                ),
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
    let database_url = required_database_url()?;
    tracing::info!(
        service = "password-vault-api",
        "running database migrations"
    );
    run_database_migrations(&database_url).await?;
    tracing::info!(
        service = "password-vault-api",
        "database migrations completed"
    );

    Ok(())
}

async fn run_synthetic_cleanup_and_exit(
    args: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut confirm = false;
    for arg in args {
        match arg.as_str() {
            "--confirm" => confirm = true,
            "--dry-run" => confirm = false,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown cleanup-synthetic argument '{arg}'; expected --dry-run or --confirm"),
                )
                .into());
            }
        }
    }

    let database_url = required_database_url()?;
    let options = SyntheticCleanupOptions::from_env(!confirm)?;
    tracing::info!(
        dry_run = options.dry_run,
        max_delete = options.max_delete,
        service = "password-vault-api",
        "running synthetic account cleanup"
    );

    let pool = db::connect(&database_url).await?;
    let report = cleanup_synthetic_accounts(&pool, &options).await?;
    pool.close().await;

    println!(
        "synthetic_cleanup dry_run={} matched={} deleted={} max_delete={}",
        report.dry_run, report.matched, report.deleted, report.max_delete
    );
    tracing::info!(
        dry_run = report.dry_run,
        matched = report.matched,
        deleted = report.deleted,
        max_delete = report.max_delete,
        service = "password-vault-api",
        "synthetic account cleanup completed"
    );

    Ok(())
}

fn required_database_url() -> Result<String, Error> {
    let database_url = env::var("PV_DATABASE_URL")
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "PV_DATABASE_URL is required"))?;
    let database_url = database_url.trim();
    if database_url.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "PV_DATABASE_URL is required",
        ));
    }
    Ok(database_url.to_string())
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
