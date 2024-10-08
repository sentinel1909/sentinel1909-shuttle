use clap::Parser;
use shuttle_backends::trace::setup_tracing;
use shuttle_common::{log::Backend, models::user::AccountTier};
use tracing::trace;

use shuttle_auth::{copy_environment, init, pgpool_init, start, sync, Args, Commands};

#[tokio::main]
async fn main() {
    setup_tracing(tracing_subscriber::registry(), Backend::Auth);

    let args = Args::parse();
    trace!(args = ?args, "parsed args");

    if let Commands::CopyPermitEnv(args) = args.command {
        copy_environment(args).await.map_err(|e| dbg!(e)).unwrap();
        return;
    }

    // do the migrations in local development
    let pool = pgpool_init(args.db_connection_uri.as_str(), cfg!(debug_assertions))
        .await
        .expect("couldn't setup the postgres connection");

    match args.command {
        Commands::Start(args) => start(pool, args).await,
        Commands::InitAdmin(args) => init(pool, args, AccountTier::Admin).await.unwrap(),
        Commands::InitDeployer(args) => init(pool, args, AccountTier::Deployer).await.unwrap(),
        Commands::Sync(args) => sync(pool, args).await.unwrap(),
        _ => unreachable!(),
    }
}
