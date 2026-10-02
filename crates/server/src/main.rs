use clap::Parser;
#[derive(Parser)]
#[command(version, about = "MoleAPI standalone API workbench server")]
struct Args {
    #[arg(long, env = "MOLEAPI_BIND", default_value = "127.0.0.1:8787")]
    bind: std::net::SocketAddr,
    #[arg(long, env = "MOLEAPI_DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long, default_value = "./data/moleapi.db")]
    database: std::path::PathBuf,
    #[arg(long, env = "MOLEAPI_SETUP_TOKEN")]
    setup_token: Option<String>,
    #[arg(long, env = "MOLEAPI_ALLOW_PRIVATE_NETWORK", default_value_t = false)]
    allow_private_network: bool,
    #[arg(long, env = "MOLEAPI_ALLOW_REGISTRATION", default_value_t = false)]
    allow_registration: bool,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let database_url = match args.database_url {
        Some(url) => url,
        None => moleapi_server::sqlite_database_url(&args.database)?,
    };
    let setup_token = args.setup_token.unwrap_or_else(|| {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        hex::encode(bytes)
    });
    let router = moleapi_server::hosted(moleapi_server::Config {
        database_url: database_url.clone(),
        setup_token: setup_token.clone(),
        allow_registration: args.allow_registration,
        allow_private_network: args.allow_private_network,
    })
    .await?;
    if moleapi_server::setup_required(&database_url).await? {
        println!("First-user setup token: {setup_token}");
    }
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    println!("MoleAPI listening on http://{}", listener.local_addr()?);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}
async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {_ = ctrl_c=>{},_ = terminate=>{}}
}
