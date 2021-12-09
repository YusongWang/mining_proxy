use std::env;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use anyhow::Result;
use log::{debug, error, info};
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

use crate::protocol::rpc::eth::{
    ClientGetWork, ClientLogin, ClientSubmitHashrate, Server, Server_id_1,
};

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

            if len > 5 {
                debug!("收到包大小 : {}", len);
                if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
                    debug!("传递给Server :{:?}", client_json_rpc);

                    w_server.write_all(&buf[0..len]).await?;
                } else {
                    debug!(
                        "Unhandle Client Msg:{:?}",
                        String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    );
                }
            }

            //io::copy(&mut dst, &mut w_server).await?;
        }
        w_server.shutdown().await
    };

    let server_to_client = async {
        let mut is_login = false;
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
            if !is_login {
                if let Ok(server_json_rpc) = serde_json::from_slice::<Server_id_1>(&buf[0..len]) {
                    debug!("登录成功 :{:?}", server_json_rpc);
                    is_login = true;
                    w_client.write_all(&buf[0..len]).await?;
                } else {
                    debug!(
                        "Pool Login Fail{:?}",
                        String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    );
                }
            } else {
                if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                    debug!("Got Job :{:?}", server_json_rpc);

                    w_client.write_all(&buf[0..len]).await?;
                } else {
                    debug!(
                        "Got Unhandle Msg:{:?}",
                        String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    );
                }
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
    let client_stream = match tls_acceptor.accept(inbound).await {
        Ok(stream) => stream,
        Err(err) => {
            error!("tls_acceptor error {:?}", err);
            panic!("")
        }
    };

    info!("tls_acceptor Success!");
    //let mut w_client = tls_acceptor.accept(inbound).await.expect("accept error");

    let addr = config
        .pool_ssl_address
        .to_socket_addrs()?
        .next()
        .ok_or("failed to resolve")
        .expect("parse address Error");
    info!("connect to {:?}", &addr);
    let socket = TcpStream::connect(&addr).await?;
    let cx = TlsConnector::builder().build()?;
    let cx = tokio_native_tls::TlsConnector::from(cx);
    info!("connectd {:?}", &addr);

    let domain: Vec<&str> = config.pool_ssl_address.split(":").collect();
    info!("{}", domain[0]);
    let mut server_stream = cx.connect(domain[0], socket).await?;

    info!("connectd {:?} with TLS", &addr);

    let (mut r_client, mut w_client) = split(client_stream);
    let (mut r_server, mut w_server) = split(server_stream);

    let client_to_server = async {
        loop {
            // parse protocol
            //let mut dst = String::new();
            let mut buf = vec![0; 1024];
            let len = r_client.read(&mut buf).await?;
            if len > 5 {
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
                    if client_json_rpc.method == "eth_submitHashrate" {
                        info!(
                            "矿机 :{} 提交本地算力 {:?}",
                            client_json_rpc.worker, client_json_rpc
                        );
                        //debug!("传递给Server :{:?}", client_json_rpc);
                    } else if client_json_rpc.method == "eth_submitHashrate" {
                        if let Some(hashrate) = client_json_rpc.params.get(0) {
                            debug!("矿机 :{} 提交本地算力 {}", client_json_rpc.worker, hashrate);
                        }
                    } else if client_json_rpc.method == "eth_submitLogin" {
                        debug!("矿机 :{} 请求登录", client_json_rpc.worker);
                    } else {
                        debug!("矿机传递未知RPC :{:?}", client_json_rpc);
                    }

                    w_server.write_all(&buf[0..len]).await?;
                } else if let Ok(client_json_rpc) =
                    serde_json::from_slice::<ClientGetWork>(&buf[0..len])
                {
                    debug!("GetWork:{:?}", client_json_rpc);
                    w_server.write_all(&buf[0..len]).await?;
                }
            }
            //io::copy(&mut dst, &mut w_server).await?;
        }
        w_server.shutdown().await
    };

    let server_to_client = async {
        let mut is_login = false;

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
            if !is_login {
                if let Ok(server_json_rpc) = serde_json::from_slice::<Server_id_1>(&buf[0..len]) {
                    debug!("登录成功 :{:?}", server_json_rpc);
                    is_login = true;
                } else {
                    debug!(
                        "Pool Login Fail{:?}",
                        String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    );
                }
            } else {
                if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                    debug!("Got Job :{:?}", server_json_rpc);

                    //w_client.write_all(&buf[0..len]).await?;
                } else {
                    debug!(
                        "Got Unhandle Msg:{:?}",
                        String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    );
                }
            }
            w_client.write_all(&buf[0..len]).await?;
            //io::copy(&mut dst, &mut w_server).await?;
        }
        //io::copy(&mut r_server, &mut w_client).await?;
        w_client.shutdown().await
    };

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
