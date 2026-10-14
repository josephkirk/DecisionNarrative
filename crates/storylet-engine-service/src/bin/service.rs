use std::{env, net::SocketAddr, path::Path};

use storylet_engine_service::service::{RuntimeService, router};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = env::var("STORYLET_ENGINE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:4317".into())
        .parse()?;
    let database_path =
        env::var("STORYLET_ENGINE_DB_PATH").unwrap_or_else(|_| "data/storylet_engine.db".into());
    if database_path != ":memory:" {
        if let Some(parent) = Path::new(&database_path)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
    }

    let app = router(RuntimeService::open(&database_path)?);
    let listener = TcpListener::bind(address).await?;
    eprintln!("storylet-engine-service listening on http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
