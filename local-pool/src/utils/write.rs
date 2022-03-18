use anyhow::{anyhow, Result};
use tokio::io::{AsyncWrite, AsyncWriteExt, WriteHalf};

const SPLIT: u8 = b'\n';

pub async fn write_to_socket_byte<W>(
    w: &mut WriteHalf<W>, mut rpc: Vec<u8>, worker: &String,
) -> Result<()>
where W: AsyncWrite {
    rpc.push(b'\n');
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        bail!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        );
    }
    Ok(())
}

pub async fn self_write_socket_byte<W>(
    w: &mut WriteHalf<W>, mut rpc: Vec<u8>, worker: &String,
) -> Result<()>
where W: AsyncWrite {
    rpc.push(SPLIT);
    let write_len = w.write(&rpc).await?;
    if write_len == 0 {
        return Err(anyhow!(
            "旷工: {} 服务器断开连接. 写入失败。远程矿池未连通！",
            worker
        ));
    }
    Ok(())
}
