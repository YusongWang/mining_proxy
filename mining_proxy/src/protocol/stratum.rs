use crate::{client::write_to_socket_byte, state::Worker};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use tokio::io::{AsyncWrite, WriteHalf};

use super::ethjson::EthClientObject;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumRoot {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumResult {
    pub id: u64,
    pub jsonrpc: String,
    pub result: Vec<bool>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumResultBool {
    pub id: u64,
    pub result: bool,
}

//{\"id\":1001,\"error\":null,\"result\":true}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumResultWorkNotify {
    pub id: u64,
    pub method: String,
    pub params: (String, String, String, bool),
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumMiningNotify {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumMiningSet {
    pub id: Value,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumErrorResult {
    pub id: i64,
    pub error: (i64, String, Value),
}

pub async fn login<W>(
    worker: &mut Worker, w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &mut String,
) -> Result<()>
where
    W: AsyncWrite,
{
    if let Some(wallet) = rpc.get_eth_wallet() {
        //rpc.set_id(CLIENT_LOGIN);
        let mut temp_worker = wallet.clone();
        let split = wallet.split(".").collect::<Vec<&str>>();
        if split.len() > 1 {
            worker.login(
                temp_worker.clone(),
                split.get(1).unwrap().to_string(),
                wallet.clone(),
            );
            *worker_name = temp_worker;
        } else {
            temp_worker.push_str(".");
            temp_worker = temp_worker + rpc.get_worker_name().as_str();
            worker.login(
                temp_worker.clone(),
                rpc.get_worker_name(),
                wallet.clone(),
            );
            *worker_name = temp_worker;
        }

        write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}
