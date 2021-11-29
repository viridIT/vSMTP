use v_smtp::config;
use v_smtp::resolver::ResolverWriteDisk;
use v_smtp::rules::rule_engine;
use v_smtp::server::ServerVSMTP;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log4rs::init_file(
        config::get::<String>("paths.logs_file").unwrap(),
        Default::default(),
    )?;

    ResolverWriteDisk::init_spool_folder(&config::get::<String>("paths.spool_dir").unwrap())?;

    let server = ServerVSMTP::<ResolverWriteDisk>::new(
        config::get::<Vec<String>>("server.addr")
            .unwrap_or_else(|_| vec![config::DEFAULT_MTA_SERVER_ADDR.to_string()])
            .into_iter()
            .filter_map(|s| match s.parse::<std::net::SocketAddr>() {
                Ok(addr) => Some(addr),
                Err(e) => {
                    log::error!("Failed to parse address from config {}", e);
                    None
                }
            })
            .collect::<Vec<_>>(),
    )?;

    rule_engine::init();

    log::warn!("Listening on: {:?}", server.addr());
    server.listen_and_serve().await
}
