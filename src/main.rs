use elsewhere::{connect, router, state, Config};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let config = Config::from_env();
    let pool = connect(&config).await.expect("database");
    let app = router(state(pool, config));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.expect("bind");
    tracing::info!("Elsewhere listening on http://localhost:5000");
    axum::serve(listener, app).await.expect("server");
}
