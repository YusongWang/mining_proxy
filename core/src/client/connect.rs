use anyhow::{bail, Result};
use tokio::io::{AsyncRead, AsyncWrite, WriteHalf};

use crate::util::config::Settings;

use super::{SSL, TCP};

// pub async fn pool_connnect(
//     config: &Settings,
// ) -> Result<(
//     tokio::io::BufReader<tokio::io::ReadHalf<Box<>>>,
//     WriteHalf<Box<dyn Asy>>,
// )> {
//     let (stream_type, pools) =
//         match crate::client::get_pool_ip_and_type_from_vec(
//             &config.share_address,
//         ) {
//             Ok(pool) => pool,
//             Err(_) => {
//                 bail!("未匹配到矿池 或 均不可链接。请修改后重试");
//             }
//         };

//     if stream_type == TCP {
//         let (outbound, _) = match crate::client::get_pool_stream(&pools) {
//             Some((stream, addr)) => (stream, addr),
//             None => {
//                 bail!("所有TCP矿池均不可链接。请修改后重试");
//             }
//         };

//         let stream = tokio::net::TcpStream::from_std(outbound)?;
//         let (pool_r, pool_w) = tokio::io::split(stream);
//         let pool_r = tokio::io::BufReader::new(pool_r);

//         Ok((pool_r, pool_w))
//     } else if stream_type == SSL {
//         let (stream, _) =
//             match crate::client::get_pool_stream_with_tls(&pools).await {
//                 Some((stream, addr)) => (stream, addr),
//                 None => {
//                     bail!("所有TCP矿池均不可链接。请修改后重试");
//                 }
//             };

//         let (pool_r, pool_w) = tokio::io::split(stream);
//         let pool_r = tokio::io::BufReader::new(pool_r);

//         Ok((pool_r, pool_w))
//     } else {
//         panic!("达到了无法达到的分支");
//     }
// }
