use anyhow::Result;
use log::info;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

mod util;
use util::{config, logger};

extern crate clap;
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};

async fn get_app_command_matches() -> Result<ArgMatches<'static>> {
     let matches = App::new(crate_name!())
          .version(crate_version!())
          .author(crate_authors!("\n"))
          .about(crate_description!())
          .arg(Arg::with_name("config")
               .short("c")
               .long("config")
               .value_name("FILE")
               .help("Sets a custom config file")
               .takes_value(true))
          .get_matches();
     Ok(matches)
}

#[tokio::main]
async fn main() -> Result<()> {
     let matches = get_app_command_matches().await?;
     let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
     let config = config::Settings::new(config_file_name)?;
     logger::init("proxy", &config.log_path, &config.log_level)?;
     info!("config init success!");

     //let tcp_ssl = tokio::spawn(async move {});
     // let tcp = tokio::spawn(async move {

     // });

     // let tcp_ssl = tokio::spawn(async move {

     // });

     tokio::join!(accept_tcp(&config));

     Ok(())
}

async fn accept_tcp(config: &config::Settings) -> Result<()> {
     
     let listener = TcpListener::bind("127.0.0.1:8082").await?;
     info!("Accepting On: 8082");

     loop {
          let (mut socket, _) = listener.accept().await?;

          tokio::spawn(async move {
               let mut buf = [0; 1024];

               // In a loop, read data from the socket and write the data back.
               loop {
                    let n = match socket.read(&mut buf).await {
                         // socket closed
                         Ok(n) if n == 0 => return,
                         Ok(n) => n,
                         Err(e) => {
                              eprintln!("failed to read from socket; err = {:?}", e);
                              return;
                         }
                    };

                    info!("got tcp package: {:?}", String::from_utf8_lossy(&buf[0..n]));

                    // Write the data back
                    if let Err(e) = socket.write_all(&buf[0..n]).await {
                         eprintln!("failed to write to socket; err = {:?}", e);
                         return;
                    }
               }
          });
     }


}
