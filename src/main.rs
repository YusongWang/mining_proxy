#![feature(test)]

mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

use anyhow::Result;

mod mode;
mod client;
mod jobs;
mod protocol;
mod state;
mod util;

use mode::{server::server_mode, client::client_mode};
use util::{
    get_app_command_matches,
};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_app_command_matches().await?;
    let _guard = sentry::init((
        "https://a9ae2ec4a77c4c03bca2a0c792d5382b@o1095800.ingest.sentry.io/6115709",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));
    
    let mode = matches.value_of("mode").unwrap_or("server");
    if mode == "server" {
        server_mode(matches).await
    } else {
        client_mode(matches).await
    }
}

