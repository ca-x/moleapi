mod args;
use args::Args;
use clap::Parser;
fn main() -> anyhow::Result<()> {
    if moleapi_server::dispatch_script_worker()? {
        return Ok(());
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run())
}
async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let database_url = match args.database_url.filter(|value| !value.is_empty()) {
        Some(url) => url,
        None => moleapi_server::sqlite_database_url(&args.database)?,
    };
    let setup_token = args
        .setup_token
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
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
