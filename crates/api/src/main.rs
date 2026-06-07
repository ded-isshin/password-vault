use password_vault_api::{ApiConfig, build_app, init_tracing};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

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
