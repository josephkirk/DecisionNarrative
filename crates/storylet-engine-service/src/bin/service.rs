use std::{env, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;

use storylet_engine_service::http::build_router;
use storylet_engine_service::mcp::run_stdio_mcp;
use storylet_engine_service::service::StudioService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let fresh_database = args.iter().any(|arg| arg == "--fresh-database");
    let default_db_path = if fresh_database {
        "data/storylet_studio_v6_clean.db"
    } else {
        "data/storylet_studio_v6.db"
    };
    let db_path = env::var("STORYLET_STUDIO_DB_PATH")
        .unwrap_or_else(|_| default_db_path.into());

    let resource_root = env::var("STORYLET_RESOURCE_ROOT").unwrap_or_else(|_| ".".into());

    // Schema v6 is a clean cutover; earlier databases are rejected without migration.
    if db_path != ":memory:" {
        if let Some(parent) = PathBuf::from(&db_path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
    }

    let service = if db_path == ":memory:" {
        StudioService::open_in_memory(resource_root)?
    } else {
        StudioService::open(&db_path, resource_root)?
    };

    if args.iter().any(|arg| arg == "--mcp") {
        return run_stdio_mcp(service).await;
    }

    let address: SocketAddr = env::var("STORYLET_STUDIO_ADDR")
        .unwrap_or_else(|_| {
            if fresh_database {
                "127.0.0.1:3001".into()
            } else {
                "127.0.0.1:3000".into()
            }
        })
        .parse()?;

    let static_dir = Some(PathBuf::from("dist/studio")).filter(|p| p.exists());

    let app = build_router(service, static_dir);
    let listener = TcpListener::bind(address).await?;
    eprintln!("storylet-engine-service listening on http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    Ok(())
}
