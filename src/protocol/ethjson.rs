use crate::{
    client::write_to_socket_byte,
    state::Worker,
    util::{config::Settings, hex_to_int},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncWrite, WriteHalf};

use super::{
    CLIENT_GETWORK, CLIENT_LOGIN, CLIENT_SUBHASHRATE, CLIENT_SUBMITWORK,
    SUBSCRIBE,
};

pub trait EthClientObject {
    fn set_id(&mut self, id: u64) -> bool;
    fn get_id(&self) -> u64;

    fn get_job_id(&mut self) -> Option<String>;
    fn get_eth_wallet(&mut self) -> Option<String>;
    fn set_wallet(&mut self, wallet: &str) -> bool;

    fn get_worker_name(&mut self) -> String;
    fn set_worker_name(&mut self, worker_name: &str) -> bool;

    fn get_submit_hashrate(&self) -> u64;
    fn set_submit_hashrate(&mut self, hash: String) -> bool;

    fn get_method(&self) -> String;
    fn is_protocol_eth_statum(&self) -> bool;

    fn to_vec(&mut self) -> Result<Vec<u8>>;
}

impl std::fmt::Debug for dyn EthClientObject + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rpc_id: {} method: {} params {} ",
            self.get_id(),
            self.get_method(),
            self.get_job_id().unwrap()
        )
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthClientRootObject {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthClientWorkerObject {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
    pub worker: String,
}

impl EthClientObject for EthClientRootObject {
    fn set_id(&mut self, id: u64) -> bool {
        self.id = id;
        true
    }

    fn get_id(&self) -> u64 { self.id }

    fn get_job_id(&mut self) -> Option<String> {
        match self.params.get(1) {
            Some(s) => Some(s.to_string()),
            None => None,
        }
    }

    fn get_eth_wallet(&mut self) -> Option<String> {
        match self.params.get(0) {
            Some(s) => Some(s.to_string()),
            None => None,
        }
    }

    fn get_worker_name(&mut self) -> String { "Default".to_string() }

    fn get_submit_hashrate(&self) -> u64 {
        if let Some(hashrate) = self.params.get(0) {
            let hashrate = match hex_to_int(&hashrate[2..hashrate.len()]) {
                Some(g) => g,
                None => match hex_to_int(&hashrate[..]) {
                    Some(h) => h,
                    None => 0,
                },
            };

            hashrate as u64
        } else {
            0
        }
    }

    fn set_worker_name(&mut self, worker_name: &str) -> bool {
        self.params[0] = worker_name.to_string();
        true
    }

    fn get_method(&self) -> String { self.method.clone() }

    fn to_vec(&mut self) -> Result<Vec<u8>> {
        let rpc = serde_json::to_vec(&self)?;
        Ok(rpc)
    }

    fn set_submit_hashrate(&mut self, hash: String) -> bool {
        self.params[0] = hash;
        true
    }

    fn is_protocol_eth_statum(&self) -> bool {
        if let Some(statum) = self.params.get(1) {
            if *statum == "EthereumStratum/1.0.0" {
                return true;
            } else {
                return false;
            }
        }

        false
    }

    fn set_wallet(&mut self, wallet: &str) -> bool {
        self.params[0] = wallet.to_string();
        true
    }
}

impl EthClientObject for EthClientWorkerObject {
    fn set_id(&mut self, id: u64) -> bool {
        self.id = id;
        true
    }

    fn get_id(&self) -> u64 { self.id }

    fn get_job_id(&mut self) -> Option<String> {
        match self.params.get(1) {
            Some(s) => Some(s.to_string()),
            None => None,
        }
    }

    fn get_eth_wallet(&mut self) -> Option<String> {
        match self.params.get(0) {
            Some(s) => Some(s.to_string()),
            None => None,
        }
    }

    fn get_worker_name(&mut self) -> String { self.worker.clone() }

    fn get_submit_hashrate(&self) -> u64 {
        if let Some(hashrate) = self.params.get(0) {
            let hashrate = match hex_to_int(&hashrate[2..hashrate.len()]) {
                Some(g) => g,
                None => match hex_to_int(&hashrate[..]) {
                    Some(h) => h,
                    None => 0,
                },
            };
            hashrate as u64
        } else {
            0
        }
    }

    fn set_worker_name(&mut self, worker_name: &str) -> bool {
        self.worker = worker_name.to_string();
        true
    }

    fn get_method(&self) -> String { self.method.clone() }

    fn to_vec(&mut self) -> Result<Vec<u8>> {
        let rpc = serde_json::to_vec(&self)?;
        Ok(rpc)
    }

    fn set_submit_hashrate(&mut self, hash: String) -> bool {
        self.params[0] = hash;
        true
    }

    fn is_protocol_eth_statum(&self) -> bool {
        if let Some(statum) = self.params.get(1) {
            if *statum == "EthereumStratum/1.0.0" {
                return true;
            } else {
                return false;
            }
        }
        false
    }

    fn set_wallet(&mut self, wallet: &str) -> bool {
        self.params[0] = wallet.to_string();
        true
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthServerRootObject {
    pub id: u64,
    pub result: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthServerRootObjectJsonRpc {
    pub id: u64,
    pub jsonrpc: String,
    pub result: Vec<String>,
}

impl EthServerRootObject {
    pub fn get_job_id(&self) -> Option<String> {
        match self.result.get(0) {
            Some(s) => Some(s.to_string()),
            None => None,
        }
    }

    pub fn get_job_result(&self) -> Option<Vec<String>> {
        if self.result.len() >= 3 {
            let res = self.result.clone();
            return Some(res);
        }
        None
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthError {
    pub code: u64,
    pub message: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthServerRootObjectBool {
    pub id: u64,
    pub jsonrpc: String,
    pub result: bool,
    pub error: EthError,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthServerRootObjectError {
    pub id: u64,
    pub jsonrpc: String,
    pub result: bool,
    pub error: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthServerRoot {
    pub id: u64,
    pub jsonrpc: String,
    pub result: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthServer {
    pub id: u64,
    pub result: bool,
}

pub async fn new_eth_submit_work<W, W2>(
    worker: &mut Worker, pool_w: &mut WriteHalf<W>,
    worker_w: &mut WriteHalf<W2>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &String,
    config: &Settings,
) -> Result<()>
where
    W: AsyncWrite,
    W2: AsyncWrite,
{
    rpc.set_id(CLIENT_SUBMITWORK);
    write_to_socket_byte(pool_w, rpc.to_vec()?, &worker_name).await
}

pub async fn new_eth_submit_hashrate<W>(
    worker: &mut Worker, w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    worker.new_submit_hashrate(rpc);
    rpc.set_id(CLIENT_SUBHASHRATE);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

pub async fn new_eth_get_work<W>(
    w: &mut WriteHalf<W>, rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.set_id(CLIENT_GETWORK);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

pub async fn new_subscribe<W>(
    w: &mut WriteHalf<W>, rpc: &mut Box<dyn EthClientObject + Send + Sync>,
    worker_name: &String,
) -> Result<()>
where
    W: AsyncWrite,
{
    rpc.set_id(SUBSCRIBE);
    write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
}

pub async fn login<W>(
    worker: &mut Worker, w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &mut String,
    config: &Settings,
) -> Result<String>
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
            let temp_full_wallet =
                config.share_wallet.clone() + "." + split[1].clone();
            // 抽取全部替换钱包
            rpc.set_wallet(&temp_full_wallet);
            *worker_name = temp_worker;
            write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await?;
            Ok(temp_full_wallet)
        } else {
            temp_worker.push_str(".");
            temp_worker = temp_worker + rpc.get_worker_name().as_str();
            worker.login(
                temp_worker.clone(),
                rpc.get_worker_name(),
                wallet.clone(),
            );
            *worker_name = temp_worker.clone();
            Ok(temp_worker)
        }
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}

pub async fn new_eth_submit_login<W>(
    worker: &mut Worker, w: &mut WriteHalf<W>,
    rpc: &mut Box<dyn EthClientObject + Send + Sync>, worker_name: &mut String,
    config: &Settings,
) -> Result<()>
where
    W: AsyncWrite,
{
    if let Some(wallet) = rpc.get_eth_wallet() {
        rpc.set_id(CLIENT_LOGIN);
        let mut temp_worker = wallet.clone();
        let mut split = wallet.split(".").collect::<Vec<&str>>();
        if split.len() > 1 {
            worker.login(
                temp_worker.clone(),
                split.get(1).unwrap().to_string(),
                wallet.clone(),
            );
            let temp_full_wallet =
                config.share_wallet.clone() + "." + split[1].clone();
            // 抽取全部替换钱包
            rpc.set_wallet(&temp_full_wallet);
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
            // 抽取全部替换钱包
            rpc.set_wallet(&config.share_wallet);
        }

        write_to_socket_byte(w, rpc.to_vec()?, &worker_name).await
    } else {
        bail!("请求登录出错。可能收到暴力攻击");
    }
}
