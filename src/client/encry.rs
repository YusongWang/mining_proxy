use anyhow::Result;
use log::info;
use tokio::io::{split, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedSender;

use crate::state::{State, Worker};
use crate::util::config::Settings;

use super::*;
pub async fn accept_en_tcp(
    worker_sender: UnboundedSender<Worker>,
    config: Settings,
    state: State,
) -> Result<()> {
    let address = format!("0.0.0.0:{}", config.encrypt_port);
    let listener = TcpListener::bind(address.clone()).await?;
    info!("😄 Accepting Encrypt On: {}", &address);

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("😄 Accepting Encrypt connection from {}", addr);

        let config = config.clone();
        let sender = worker_sender.clone();
        let state = state.clone();
        tokio::spawn(async move { transfer(sender, stream, &config, state).await });
    }
}

async fn transfer(
    worker_sender: UnboundedSender<Worker>,
    tcp_stream: TcpStream,
    config: &Settings,
    state: State,
) -> Result<()> {
    let (worker_r, worker_w) = split(tcp_stream);
    let worker_r = BufReader::new(worker_r);
    let (stream_type, pools) = match crate::client::get_pool_ip_and_type(&config) {
        Some(pool) => pool,
        None => {
            info!("未匹配到矿池 或 均不可链接。请修改后重试");
            return Ok(());
        }
    };

    if stream_type == crate::client::TCP {
        handle_tcp_pool(
            worker_sender,
            worker_r,
            worker_w,
            &pools,
            &config,
            state,
            true,
        )
        .await
    } else if stream_type == crate::client::SSL {
        handle_tls_pool(
            worker_sender,
            worker_r,
            worker_w,
            &pools,
            &config,
            state,
            true,
        )
        .await
    } else {
        log::error!("致命错误：未找到支持的矿池BUG 请上报");
        return Ok(());
    }
}
