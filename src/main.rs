use anyhow::Result;
use referral_system::{AppState, Config, init_pool, init_router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let pool = init_pool(&config.database_url).await?;
    let app = init_router(AppState {
        pool,
        config: config.clone(),
    });

    let port = config.server_port;
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = TcpListener::bind(addr).await?;

    println!("Listening on 0.0.0.0:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
