use anyhow::{bail, Result};
use std::net::TcpStream;

// const POOLS:Vec<String> =  vec![
//     "47.242.58.242:8080".to_string(),
//     "47.242.58.242:8080".to_string(),
// ];

// const POOLS:Vec<String> = vec![
//     "asia2.ethermine.org:4444".to_string(),
//     "asia1.ethermine.org:4444".to_string(),
//     "asia2.ethermine.org:14444".to_string(),
//     "asia1.ethermine.org:14444".to_string(),
// ];

pub async fn get_develop_pool_stream() -> Result<TcpStream> {
    cfg_if::cfg_if! {
        if #[cfg(debug_assertions)] {
            let pools = vec![
                "47.242.58.242:8080".to_string(),
                "47.242.58.242:8080".to_string(),
            ];
        }  else {
            let pools = vec![
                "asia2.ethermine.org:4444".to_string(),
                "asia1.ethermine.org:4444".to_string(),
                "asia2.ethermine.org:14444".to_string(),
                "asia1.ethermine.org:14444".to_string(),
            ];
        }
    }

    let (stream, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    Ok(stream)
}

pub async fn get_proxy_pool_stream(config: &crate::util::config::Settings) -> Result<TcpStream> {
    cfg_if::cfg_if! {
        if #[cfg(debug_assertions)] {
            let pools = vec![
                "47.242.58.242:8080".to_string(),
                "47.242.58.242:8080".to_string(),
            ];
        }  else {
            let pools = vec![
                "asia2.ethermine.org:4444".to_string(),
                "asia1.ethermine.org:4444".to_string(),
                "asia2.ethermine.org:14444".to_string(),
                "asia1.ethermine.org:14444".to_string(),
            ];
        }
    }

    let (stream, _) = match crate::client::get_pool_stream(&pools) {
        Some((stream, addr)) => (stream, addr),
        None => {
            bail!("所有TCP矿池均不可链接。请修改后重试");
        }
    };

    Ok(stream)
}
