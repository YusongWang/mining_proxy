use std::net::ToSocketAddrs;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{bail, Result};
use log::info;

use tokio::io::split;
use tokio::net::{TcpListener, TcpStream};
extern crate native_tls;
use native_tls::{Identity, TlsConnector};

use futures::FutureExt;
use tokio::sync::mpsc::{Sender, UnboundedSender};
use tokio::sync::{broadcast, RwLock};
use tokio_native_tls::TlsStream;

use crate::client::{client_to_server, server_to_client};

use crate::protocol::rpc::eth::ServerId1;
use crate::state::State;
use crate::util::config::Settings;

pub async fn accept_tcp_with_tls(
    state: Arc<RwLock<State>>,
    config: Settings,
    job_send: broadcast::Sender<String>,
    proxy_fee_sender: UnboundedSender<String>,
    fee_send: UnboundedSender<String>,
    state_send: UnboundedSender<String>,
    dev_state_send: UnboundedSender<String>,
    cert: Identity,
) -> Result<()> {
    if config.pool_ssl_address.is_empty() {
        return Ok(());
    }

    let address = format!("0.0.0.0:{}", config.ssl_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("😄 Accepting Tls On: {}", &address);

    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build()?);
    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("😄 accept connection from {}", addr);

        let c = config.clone();
        let acceptor = tls_acceptor.clone();
        let proxy_fee_sender = proxy_fee_sender.clone();
        let fee = fee_send.clone();
        let state = state.clone();
        let jobs_recv = job_send.subscribe();
        let state_send = state_send.clone();
        let dev_state_send = dev_state_send.clone();
        tokio::spawn(async move {
            let transfer = transfer_ssl(
                state,
                jobs_recv,
                acceptor,
                stream,
                c,
                proxy_fee_sender,
                fee,
                state_send,
                dev_state_send,
            )
            .map(|r| {
                if let Err(e) = r {
                    info!("❎ 线程退出 : error={}", e);
                }
            });

            tokio::spawn(transfer);
        });
    }
}

async fn transfer_ssl(
    state: Arc<RwLock<State>>,
    jobs_recv: broadcast::Receiver<String>,
    tls_acceptor: tokio_native_tls::TlsAcceptor,
    inbound: TcpStream,
    config: Settings,
    proxy_fee_sender: UnboundedSender<String>,
    fee: UnboundedSender<String>,
    state_send: UnboundedSender<String>,
    dev_state_send: UnboundedSender<String>,
) -> Result<()> {
    let client_stream = tls_acceptor.accept(inbound).await?;
    info!("😄 tls_acceptor Success!");

    let (stream, addr) = match crate::util::get_pool_stream_with_tls(&config.pool_ssl_address).await {
        Some((stream, addr)) => (stream, addr),
        None => {
            info!("所有SSL矿池均不可链接。请修改后重试");
            std::process::exit(100);
        }
    };


    let (r_client, w_client) = split(client_stream);
    let (r_server, w_server) = split(stream);
    use tokio::sync::mpsc;
    //let (tx, mut rx): ServerId1 = mpsc::unbounded_channel();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerId1>();
    let mut worker = Arc::new(RwLock::new(String::new()));

    tokio::try_join!(
        client_to_server(
            state.clone(),
            worker.clone(),
            config.clone(),
            r_client,
            w_server,
            proxy_fee_sender.clone(),
            //state_send.clone(),
            fee.clone(),
            tx.clone()
        ),
        server_to_client(
            state.clone(),
            worker.clone(),
            config.clone(),
            jobs_recv,
            r_server,
            w_client,
            proxy_fee_sender.clone(),
            state_send.clone(),
            dev_state_send.clone(),
            rx
        )
    )?;

    // let client_to_server = async {
    //     loop {
    //         // parse protocol
    //         //let mut dst = String::new();
    //         let mut buf = vec![0; 1024];
    //         let len = r_client.read(&mut buf).await?;
    //         if len == 0 {
    //             info!("客户端断开连接.");
    //             return w_server.shutdown().await;
    //         }

    //         if len > 5 {
    //             debug!("收到包大小 : {}", len);

    //             if let Ok(client_json_rpc) = serde_json::from_slice::<Client>(&buf[0..len]) {
    //                 if client_json_rpc.method == "eth_submitWork" {
    //                     info!(
    //                         "矿机 :{} Share #{:?}",
    //                         client_json_rpc.worker, client_json_rpc.id
    //                     );
    //                     //debug!("传递给Server :{:?}", client_json_rpc);
    //                 } else if client_json_rpc.method == "eth_submitHashrate" {
    //                     if let Some(hashrate) = client_json_rpc.params.get(0) {
    //                         debug!("矿机 :{} 提交本地算力 {}", client_json_rpc.worker, hashrate);
    //                     }
    //                 } else if client_json_rpc.method == "eth_submitLogin" {
    //                     debug!("矿机 :{} 请求登录", client_json_rpc.worker);
    //                 } else {
    //                     debug!("矿机传递未知RPC :{:?}", client_json_rpc);
    //                 }

    //                 w_server.write_all(&buf[0..len]).await?;
    //             } else if let Ok(client_json_rpc) =
    //                 serde_json::from_slice::<ClientGetWork>(&buf[0..len])
    //             {
    //                 debug!("GetWork:{:?}", client_json_rpc);
    //                 w_server.write_all(&buf[0..len]).await?;
    //             }
    //         }
    //         //io::copy(&mut dst, &mut w_server).await?;
    //     }
    // };

    // let server_to_client = async {
    //     let mut is_login = false;

    //     loop {
    //         let mut buf = vec![0; 1024];
    //         let len = r_server.read(&mut buf).await?;
    //         if len == 0 {
    //             info!("服务端断开连接.");
    //             return w_client.shutdown().await;
    //         }

    //         debug!("收到包大小 : {}", len);

    //         if !is_login {
    //             if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
    //                 debug!("登录成功 :{:?}", server_json_rpc);
    //                 is_login = true;
    //             } else {
    //                 debug!(
    //                     "Pool Login Fail{:?}",
    //                     String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
    //                 );
    //             }
    //         } else {
    //             if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
    //                 debug!("Got Job :{:?}", server_json_rpc);

    //                 //w_client.write_all(&buf[0..len]).await?;
    //             } else {
    //                 debug!(
    //                     "Got Unhandle Msg:{:?}",
    //                     String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
    //                 );
    //             }
    //         }
    //         let len = w_client.write(&buf[0..len]).await?;
    //         if len == 0 {
    //             info!("服务端写入失败 断开连接.");
    //             return w_client.shutdown().await;
    //         }
    //     }
    // };

    // tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
