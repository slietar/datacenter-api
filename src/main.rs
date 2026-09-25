#![allow(unused)]

mod binary_cache;
mod client;
mod comin;
mod config;
mod handlers;
mod headscale;
mod ipmi;
mod live_state;
mod manager;
mod nar;
mod polling_state;
mod pxe;
mod state;
mod store;

mod protobuf {
    tonic::include_proto!("protobuf");
}

use axum::Router;
use axum::routing::{get, put};
use axum_extra::middleware::option_layer;
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

#[derive(rust_embed::RustEmbed, Clone)]
#[folder = "web/dist"]
struct Assets;

use tower_http::cors::{self, CorsLayer};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,

    #[arg(short, long, env = "PORT", default_value_t = 8080)]
    port: u16,

    #[arg(long)]
    cors_allow_all: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("h2=info".parse().unwrap())
                .add_directive("tower=info".parse().unwrap())
                .add_directive("tower_http=warn".parse().unwrap()),
        )
        .compact()
        .init();

    let args = Cli::parse();

    let config = std::fs::read(args.config)?;
    let mut config: Config = toml::from_slice(&config)?;

    match (&config.ipmi.password, &config.ipmi.password_file) {
        (Some(_), None) => (),
        (None, Some(path)) => {
            let password = std::fs::read_to_string(path)?;
            config.ipmi.password = password.trim_end_matches('\n').to_owned().into();
        }
        (None, None) => anyhow::bail!("Either `password` or `password_file` must be provided"),
        (Some(_), Some(_)) => anyhow::bail!("Cannot set both `password` and `password_file`"),
    }

    let host_managers = config
        .host
        .iter()
        .map(|(host_name, host_config)| {
            let manager = manager::HostManager::new(host_name, &config, host_config);
            (host_name.clone(), manager)
        })
        .collect::<HashMap<_, _>>();

    let state = state::State {
        headscale_manager: config.headscale.as_ref().map(|headscale| {
            headscale::manager::HeadscaleManager::new(
                headscale.url.clone(),
                headscale.api_key.clone(),
            )
        }),
        host_managers: Arc::new(host_managers),
    };

    let serve_assets = axum_embed::ServeEmbed::<Assets>::new();
    let app = Router::new()
        // .route("/whoami", get(whoami_handler))
        // .route("/hosts", get(ipmi_hosts_handler))
        .route("/host/{hostname}", get(handlers::host_get_handler))
        .route("/host/{hostname}/events", get(handlers::host_sse_handler))
        // .route("/host/{hostname}/command", put(ipmi_host_put_handler))
        // .nest("/pxe", pxe::router(config.clone()))
        .fallback_service(serve_assets)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(option_layer(
            args.cors_allow_all
                .then(|| CorsLayer::new().allow_origin(cors::Any)),
        ))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", args.port)).await?;
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await?;
    Ok(())
}
