use log::info;
use serde::Serialize;
use std::collections::HashMap;
use tokio::io::{AsyncWrite, WriteHalf};

use crate::client::*;

pub async fn send_job_to_client<T, W>(
    state: i32,
    job_rpc: T,
    send_jobs: &mut HashMap<String, (u64, u64)>,
    w: &mut WriteHalf<W>,
    worker: &String,
) -> Option<()>
where
    T: crate::protocol::rpc::eth::ServerRpc + Serialize,
    W: AsyncWrite,
{
    if state != 2 {
        return None;
    }
    if let Some(job_id) = job_rpc.get_job_id() {
        let diff = job_rpc.get_diff();
        send_jobs.insert(job_id, (0, diff));
        info!("发送抽水JOB");
        match write_to_socket(w, &job_rpc, &worker).await {
            Ok(_) => return Some(()),
            Err(_) => return None,
        }
    }

    None
}
