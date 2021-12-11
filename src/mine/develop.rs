use std::net::ToSocketAddrs;

use crate::{
    protocol::rpc::eth::{Client, ClientGetWork, Server, ServerId1},
    util::config::Settings,
};
use anyhow::Result;

use bytes::{BufMut, BytesMut};
//use log::{debug, info};
use native_tls::TlsConnector;
use tokio::{
    io::{split, AsyncRead, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
    time::sleep,
};

#[derive(Debug)]
pub struct Mine {
    config: Settings,
    hostname: String,
    wallet: String,
}

impl Mine {
    pub async fn new(config: Settings, wallet: String) -> Result<Self> {
        let name = hostname::get()?;
        let mut hostname = String::new();
        if name.is_empty() {
            hostname = "proxy_wallet_mine".into();
        } else {
            hostname = hostname + name.to_str().unwrap();
        }

        Ok(Self {
            config,
            hostname: hostname,
            wallet: wallet,
        })
    }

    // pub async fn accept(&self, send: Sender<String>, mut recv: Receiver<String>) {
    //     if self.config.share == 1 {
    //         info!("✅✅ 开启TCP矿池抽水{}",self.config.share_tcp_address);
    //         self.accept_tcp(send, recv)
    //             .await
    //             .expect("❎❎ TCP 抽水线程启动失败");
    //     } else if self.config.share == 2 {
    //         info!("✅✅ 开启TLS矿池抽水{}",self.config.share_ssl_address);
    //         self.accept_tcp_with_tls(send, recv)
    //             .await
    //             .expect("❎❎ TLS 抽水线程启动失败");
    //     } else {
    //         info!("✅✅ 未开启抽水");
    //     }
    // }

    // async fn accept_tcp(&self, send: Sender<String>, mut recv: Receiver<String>) -> Result<()> {
    //     let mut outbound = TcpStream::connect(&self.config.share_tcp_address.to_string()).await?;
    //     let (mut r_server, mut w_server) = split(outbound);

    //     // { id: 40, method: "eth_submitWork", params: ["0x5fcef524222c218e", "0x5dc7070a672a9b432ec76075c1e06cccca9359d81dc42a02c7d80f90b7e7c20c", "0xde91884821ac90d583725a85d94c68468c0473f49a0907f45853578b9c617e0e"], worker: "P0001" }
    //     // { id: 6, method: "eth_submitHashrate", params: ["0x1dab657b", "a5f9ff21c5d98fbe3d08bf733e2ac47c0650d198bd812743684476d4d98cdf32"], worker: "P0001" }

    //     tokio::try_join!(
    //         self.login_and_getwork(send),
    //         self.client_to_server(w_server, recv),
    //         self.server_to_client(r_server)
    //     )?;
    //     Ok(())
    // }

    pub async fn accept_tcp_with_tls(
        &self,
        send: Sender<String>,
        mut recv: Receiver<String>,
    ) -> Result<()> {
        let addr = "asia2.ethermine.org:5555"
            .to_socket_addrs()?
            .next()
            .ok_or("failed to resolve")
            .expect("parse address Error");

        //info!("✅✅ connect to {:?}", &addr);
        let socket = TcpStream::connect(&addr).await?;
        let cx = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()?;
        let cx = tokio_native_tls::TlsConnector::from(cx);
        //info!("✅✅ connectd {:?}", &addr);

        //let domain: Vec<&str> = "asia2.ethermine.org:5555".split(":").collect();
        let server_stream = cx
            .connect("asia2.ethermine.org", socket)
            .await
            .expect("❗❎ 与矿池SSL握手失败");

        let (mut r_server, mut w_server) = split(server_stream);

        tokio::try_join!(
            self.login_and_getwork(send),
            self.client_to_server(w_server, recv),
            self.server_to_client(r_server)
        )?;
        Ok(())
    }

    async fn server_to_client<R>(&self, mut r: ReadHalf<R>) -> Result<(), std::io::Error>
    where
        R: AsyncRead,
    {
        let mut is_login = false;

        loop {
            let mut buf = vec![0; 1024];
            let len = r.read(&mut buf).await.expect("从服务器读取失败.");
            if len == 0 {
                //info!("服务端断开连接.");
                return Ok(());
                //return w_server.shutdown().await;
            }

            if !is_login {
                if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                    if server_json_rpc.result == false {
                        panic!("❗❎ 矿池登录失败，请尝试重启程序");
                    }

                    //info!("✅✅ 登录成功 :{:?}", server_json_rpc);
                    is_login = true;
                } else {
                    panic!("❗❎ 矿池登录失败，请尝试重启程序");
                    // debug!(
                    //     "❗❎ 登录失败{:?}",
                    //     String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    // );
                    //return w_server.shutdown().await;
                }
            } else {
                if let Ok(server_json_rpc) = serde_json::from_slice::<ServerId1>(&buf[0..len]) {
                    //debug!("收到开发者抽水矿机返回 {:?}",server_json_rpc);
                    // if (server_json_rpc.id == 6) {
                    //     info!("🚜🚜 算力提交成功");
                    // } else {
                    //     info!("👍👍 Share Accept");
                    // }
                } else if let Ok(server_json_rpc) = serde_json::from_slice::<Server>(&buf[0..len]) {
                    // debug!("Got jobs {:?}",server_json_rpc);
                    // if let Some(diff) = server_json_rpc.result.get(3) {
                    //     debug!("✅ Got Job Diff {}", diff);
                    // }
                } else {
                    // debug!(
                    //     "❗ ------未捕获封包:{:?}",
                    //     String::from_utf8(buf.clone()[0..len].to_vec()).unwrap()
                    // );
                }
            }
        }
        Ok(())
    }

    async fn client_to_server<W>(
        &self,
        mut w: WriteHalf<W>,
        mut recv: Receiver<String>,
    ) -> Result<(), std::io::Error>
    where
        W: AsyncWriteExt,
    {
        loop {
            let client_msg = recv.recv().await.expect("Channel Close");
            //debug!("🚜🚜 抽水矿机 :{}", client_msg);

            if let Ok(mut client_json_rpc) = serde_json::from_slice::<Client>(client_msg.as_bytes())
            {
                if client_json_rpc.method == "eth_submitWork" {
                    client_json_rpc.id = 40;
                    client_json_rpc.worker = self.hostname.clone();

                    // info!(
                    //     "✅✅ 矿机 :{} Share #{:?}",
                    //     client_json_rpc.worker, client_json_rpc.id
                    // );
                } else if client_json_rpc.method == "eth_submitHashrate" {
                    if let Some(hashrate) = client_json_rpc.params.get(0) {
                        //debug!("✅✅ 矿机 :{} 提交本地算力 {}", client_json_rpc.worker, hashrate);
                    }
                } else if client_json_rpc.method == "eth_submitLogin" {
                    //debug!("✅✅ 矿机 :{} 请求登录", client_json_rpc.worker);
                } else {
                    //debug!("矿机传递未知RPC :{:?}", client_json_rpc);
                }

                let rpc = serde_json::to_vec(&client_json_rpc)?;
                let mut byte = BytesMut::new();
                byte.put_slice(&rpc[0..rpc.len()]);
                byte.put_u8(b'\n');
                let w_len = w.write_buf(&mut byte).await?;
                if w_len == 0 {
                    return w.shutdown().await;
                }
            } else if let Ok(client_json_rpc) =
                serde_json::from_slice::<ClientGetWork>(client_msg.as_bytes())
            {
                let rpc = serde_json::to_vec(&client_json_rpc)?;
                let mut byte = BytesMut::new();
                byte.put_slice(&rpc[0..rpc.len()]);
                byte.put_u8(b'\n');
                let w_len = w.write_buf(&mut byte).await?;
                if w_len == 0 {
                    return w.shutdown().await;
                }
            }
        }
    }

    async fn login_and_getwork(&self, send: Sender<String>) -> Result<(), std::io::Error> {
        let login = Client {
            id: 1,
            method: "eth_submitLogin".into(),
            params: vec![self.wallet.clone(), "x".into()],
            worker: self.hostname.clone(),
        };
        let login_msg = serde_json::to_string(&login)?;
        send.send(login_msg).await.expect("异常退出了.");

        sleep(std::time::Duration::new(10,0)).await;

        let eth_get_work = ClientGetWork {
            id: 5,
            method: "eth_getWork".into(),
            params: vec![],
        };

        let eth_get_work_msg = serde_json::to_string(&eth_get_work)?;
        send.send(eth_get_work_msg).await.expect("异常退出了.");

        loop {
            //计算速率
            let submit_hashrate = Client {
                id: 6,
                method: "eth_submitHashrate".into(),
                params: vec!["0x5e500000".into(), "x".into()],
                worker: self.hostname.clone(),
            };

            let submit_hashrate_msg = serde_json::to_string(&submit_hashrate)?;
            send.send(submit_hashrate_msg).await.expect("异常退出了.");

            sleep(std::time::Duration::new(20,0)).await;
        }
    }
}
