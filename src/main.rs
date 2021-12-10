use anyhow::Result;
use clap::crate_name;

use log::info;

mod client;
mod util;

use util::{config, get_app_command_matches, logger};
mod protocol;
use client::{tcp, tls};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_app_command_matches().await?;
    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = config::Settings::new(config_file_name)?;
    logger::init(crate_name!(), config.log_path.clone(), config.log_level)?;
    info!("✅ config init success!");

    let _ = tokio::join!(
        tcp::accept_tcp(config.clone()),
        tls::accept_tcp_with_tls(config.clone())
    );

    Ok(())
}
