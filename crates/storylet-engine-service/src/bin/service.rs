use std::{env, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;

use storylet_engine_service::legacy_import::maybe_import_legacy_database;
use storylet_engine_service::mcp::run_stdio_mcp;
use storylet_engine_service::service::StudioService;
use storylet_engine_service::http::build_router;
use storylet_engine_service::store::Store;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let db_path = env::var("STORYLET_STUDIO_DB_PATH")
        .or_else(|_| env::var("STORYLET_ENGINE_DB_PATH"))
        .or_else(|_| env::var("STORYLET_DB_PATH"))
        .unwrap_or_else(|_| "data/storylet_studio_v2.db".into());

    let resource_root = env::var("STORYLET_RESOURCE_ROOT")
        .unwrap_or_else(|_| ".".into());

    let strata_url = env::var("STRATA_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/v1".into());

    let strata_model = env::var("STRATA_MODEL")
        .unwrap_or_else(|_| "strata-uncensored".into());

    let embedding_url = env::var("EMBEDDING_SERVICE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4318".into());

    // Run one-time migration if needed
    if db_path != ":memory:" {
        if let Some(parent) = PathBuf::from(&db_path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        if let Ok(mut store) = Store::open(&db_path) {
            let _ = maybe_import_legacy_database(&mut store, "data/storylet_studio.db", "data/storylet_engine.db");
        }
    }

    let service = if db_path == ":memory:" {
        StudioService::open_in_memory(resource_root, strata_url, strata_model, embedding_url)?
    } else {
        StudioService::open(&db_path, resource_root, strata_url, strata_model, embedding_url)?
    };

    if args.iter().any(|arg| arg == "--mcp") {
        return run_stdio_mcp(service).await;
    }

    let address: SocketAddr = env::var("STORYLET_ENGINE_ADDR")
        .or_else(|_| env::var("STORYLET_STUDIO_ADDR"))
        .or_else(|_| env::var("STORYLET_WEB_PORT").map(|p| format!("127.0.0.1:{p}")))
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()?;

    let static_dir = Some(PathBuf::from("dist/studio"))
        .filter(|p| p.exists())
        .or_else(|| Some(PathBuf::from("public")));

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
