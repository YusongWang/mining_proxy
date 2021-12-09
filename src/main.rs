use std::env;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use anyhow::Result;
use log::{debug, info};
use tokio::fs::File;
use tokio::io::{self, split, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
extern crate native_tls;

use native_tls::{Identity, TlsAcceptor, TlsConnector, TlsStream};

use futures::FutureExt;

mod util;

use util::config::Settings;
use util::{config, logger};
mod protocol;

use protocol::rpc::eth::Client;

extern crate clap;
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};

use crate::protocol::rpc::eth::Server;

async fn get_app_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(crate_description!())
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .get_matches();
    Ok(matches)
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = get_app_command_matches().await?;
    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = config::Settings::new(config_file_name)?;
    logger::init("proxy", config.log_path.clone(), config.log_level)?;
    info!("config init success!");

    //let tcp_ssl = tokio::spawn(async move {});
    // let tcp = tokio::spawn(async move {

    // });

    // let tcp_ssl = tokio::spawn(async move {

    // });

    tokio::join!(
        accept_tcp(config.clone()),
        accept_tcp_with_tls(config.clone())
    );

    Ok(())
}

async fn accept_tcp_with_tls(config: Settings) -> Result<()> {
    // let mut file = File::open("identity.pfx").await?;
    // let mut identity = vec![];
    // file.read_to_end(&mut identity).await?;
    // let identity = Identity::from_pkcs12(&identity, "hunter2")?;

    // let acceptor = TlsAcceptor::new(identity)?;
    // let acceptor = Arc::new(acceptor);

    let address = format!("0.0.0.0:{}", config.ssl_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("Accepting Tls On: {}", &address);

    let der = include_bytes!("identity.p12");
    let cert = Identity::from_pkcs12(der, "mypass")?;
    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build()?);
    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("accept connection from {}", addr);

        let c = config.clone();
        let acceptor = tls_acceptor.clone();

        tokio::spawn(async move {
            let transfer = transfer_ssl(acceptor, stream, c).map(|r| {
                if let Err(e) = r {
                    println!("Failed to transfer; error={}", e);
                }
            });

            tokio::spawn(transfer);
        });
    }

    Ok(())
    //     //let (mut socket, _) = listener.accept().await?;
    //     while let Ok((inbound, _)) = listener.accept().await {
    //         tokio::spawn(async move {
    //             let server_addr = env::args()
    //                 .nth(2)
    //                 .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    //             let transfer = transfer(inbound, server_addr.clone()).map(|r| {
    //                 if let Err(e) = r {
    //                     println!("Failed to transfer; error={}", e);
    //                 }
    //             });

    //             tokio::spawn(transfer);
    //         });
    //     }

    //     info!("exit!");
    //     Ok(())
    // tokio::spawn(async move {
    //      let mut buf = [0; 1024];

    //      // In a loop, read data from the socket and write the data back.
    //      loop {
    //           let n = match socket.read(&mut buf).await {
    //                // socket closed
    //                Ok(n) if n == 0 => return,
    //                Ok(n) => n,
    //                Err(e) => {
    //                     eprintln!("failed to read from socket; err = {:?}", e);
    //                     return;
    //                }
    //           };

    //           info!("got tcp package: {:?}", String::from_utf8_lossy(&buf[0..n]));

    //           // Write the data back
    //           if let Err(e) = socket.write_all(&buf[0..n]).await {
    //                eprintln!("failed to write to socket; err = {:?}", e);
    //                return;
    //           }
    //      }
    // });
}
async fn accept_tcp(config: Settings) -> Result<()> {
    let address = format!("0.0.0.0:{}", config.tcp_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("Accepting On: {}", &address);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("accept connection from {}", addr);
        let c = config.clone();
        
        tokio::spawn(async move {
            let transfer = transfer(stream, c).map(|r| {
                if let Err(e) = r {
                    println!("Failed to transfer; error={}", e);
                }
            });

            tokio::spawn(transfer);
        });
    }

    Ok(())
    //     //let (mut socket, _) = listener.accept().await?;
    //     while let Ok((inbound, _)) = listener.accept().await {
    //         tokio::spawn(async move {
    //             let server_addr = env::args()
    //                 .nth(2)
    //                 .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    //             let transfer = transfer(inbound, server_addr.clone()).map(|r| {
    //                 if let Err(e) = r {
    //                     println!("Failed to transfer; error={}", e);
    //                 }
    //             });

    //             tokio::spawn(transfer);
    //         });
    //     }

    //     info!("exit!");
    //     Ok(())
    // tokio::spawn(async move {
    //      let mut buf = [0; 1024];

    //      // In a loop, read data from the socket and write the data back.
    //      loop {
    //           let n = match socket.read(&mut buf).await {
    //                // socket closed
    //                Ok(n) if n == 0 => return,
    //                Ok(n) => n,
    //                Err(e) => {
    //                     eprintln!("failed to read from socket; err = {:?}", e);
    //                     return;
    //                }
    //           };

    //           info!("got tcp package: {:?}", String::from_utf8_lossy(&buf[0..n]));

    //           // Write the data back
    //           if let Err(e) = socket.write_all(&buf[0..n]).await {
    //                eprintln!("failed to write to socket; err = {:?}", e);
    //                return;
    //           }
    //      }
    // });
}

async fn transfer(mut inbound: TcpStream, config: Settings) -> Result<()> {
    let mut outbound = TcpStream::connect(&config.pool_tcp_address.to_string()).await?;

    let (mut r_client, mut w_client) = inbound.split();
    let (mut r_server, mut w_server) = outbound.split();

    let client_to_server = async {
        loop {
            // parse protocol
            //let mut dst = String::new();
            let mut buf = vec![0; 1024];
            let len = r_client.read(&mut buf).await?;
            debug!("收到包大小 : {}", len);
            // match serde_json::from_slice::<Client>(&buf[0..len]){
            //     Ok(_) => todo!(),
            //     Err(_) => todo!(),
            // }
            // if let client_json_rpc = match serde_json::from_slice(&buf[0..len]) {
            //      Ok(client) => client,
            //      Err(e) => {
            //           debug!("Unpackage :{:?}", &buf[0..len]);
            //      },
            // }
            if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
                debug!("传递给Server :{:?}", client_json_rpc);

                w_server.write_all(&buf[0..len]).await?;
            } else {
                debug!(
                    "Unhandle Client Msg:{:?}",
                    String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                );
            }

            //io::copy(&mut dst, &mut w_server).await?;
        }
        w_server.shutdown().await
    };

    let server_to_client = async {
        loop {
            // parse protocol
            //let mut dst = String::new();
            let mut buf = vec![0; 1024];
            let len = r_server.read(&mut buf).await?;
            debug!("收到包大小 : {}", len);
            // match serde_json::from_slice::<Client>(&buf[0..len]){
            //     Ok(_) => todo!(),
            //     Err(_) => todo!(),
            // }
            // if let client_json_rpc = match serde_json::from_slice(&buf[0..len]) {
            //      Ok(client) => client,
            //      Err(e) => {
            //           debug!("Unpackage :{:?}", &buf[0..len]);
            //      },
            // }
            if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                debug!("传递给Client :{:?}", server_json_rpc);

                w_client.write_all(&buf[0..len]).await?;
            } else {
                debug!(
                    "Unhandle Client Msg:{:?}",
                    String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                );
            }

            //io::copy(&mut dst, &mut w_server).await?;
        }
        //io::copy(&mut r_server, &mut w_client).await?;
        w_client.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}

async fn transfer_ssl(
    tls_acceptor: tokio_native_tls::TlsAcceptor,
    mut inbound: TcpStream,
    config: Settings,
) -> Result<()> {
    let mut client_stream = tls_acceptor.accept(inbound).await.expect("accept error");
    //let mut w_client = tls_acceptor.accept(inbound).await.expect("accept error");

    let addr = config
        .pool_ssl_address
        .to_socket_addrs()?
        .next()
        .ok_or("failed to resolve www.rust-lang.org")
        .expect("parse address Error");

    let socket = TcpStream::connect(&addr).await?;
    let cx = TlsConnector::builder().build()?;
    let cx = tokio_native_tls::TlsConnector::from(cx);

    let mut server_stream = cx.connect(config.pool_ssl_address.as_str(), socket).await?;

    let (mut r_client, mut w_client) = split(client_stream);
    let (mut r_server, mut w_server) = split(server_stream);

    let client_to_server = async {
        loop {
            // parse protocol
            //let mut dst = String::new();
            let mut buf = vec![0; 1024];
            let len = r_client.read(&mut buf).await?;
            debug!("收到包大小 : {}", len);
            // match serde_json::from_slice::<Client>(&buf[0..len]){
            //     Ok(_) => todo!(),
            //     Err(_) => todo!(),
            // }
            // if let client_json_rpc = match serde_json::from_slice(&buf[0..len]) {
            //      Ok(client) => client,
            //      Err(e) => {
            //           debug!("Unpackage :{:?}", &buf[0..len]);
            //      },
            // }
            if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
                debug!("传递给Server :{:?}", client_json_rpc);

                w_server.write_all(&buf[0..len]).await?;
            } else {
                debug!(
                    "Unhandle Client Msg:{:?}",
                    String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                );
            }

            //io::copy(&mut dst, &mut w_server).await?;
        }
        w_server.shutdown().await
    };

    let server_to_client = async {
        loop {
            // parse protocol
            //let mut dst = String::new();
            let mut buf = vec![0; 1024];
            let len = r_server.read(&mut buf).await?;
            debug!("收到包大小 : {}", len);
            // match serde_json::from_slice::<Client>(&buf[0..len]){
            //     Ok(_) => todo!(),
            //     Err(_) => todo!(),
            // }
            // if let client_json_rpc = match serde_json::from_slice(&buf[0..len]) {
            //      Ok(client) => client,
            //      Err(e) => {
            //           debug!("Unpackage :{:?}", &buf[0..len]);
            //      },
            // }
            if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                debug!("传递给Client :{:?}", server_json_rpc);

                w_client.write_all(&buf[0..len]).await?;
            } else {
                debug!(
                    "Unhandle Client Msg:{:?}",
                    String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                );
            }

            //io::copy(&mut dst, &mut w_server).await?;
        }
        //io::copy(&mut r_server, &mut w_client).await?;
        w_client.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
