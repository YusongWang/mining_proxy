use crate::util::hex_to_int;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait ServerRpc {
    fn set_result(&mut self, res: Vec<std::string::String>) -> bool;
    fn set_diff(&mut self, diff: String) -> bool;
    fn get_diff(&self) -> u64;
    fn get_job_id(&self) -> Option<String>;
}

pub trait ClientRpc {
    fn set_id(&mut self, id: u64) -> bool;
    fn get_id(&mut self) -> u64;

    fn get_job_id(&mut self) -> Option<String>;
    fn get_wallet(&mut self) -> Option<String>;

    fn get_worker_name(&mut self) -> String;
    fn set_worker_name(&mut self, worker_name: &str) -> bool;

    fn get_submit_hashrate(&self) -> u64;
}

//{\"id\":1,\"method\":\"eth_submitLogin\",\"params\":[\"0x98be5c44d574b96b320dffb0ccff116bda433b8e\",\"x\"],\"worker\":\"P0002\"}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

impl ClientRpc for Client {
    fn set_id(&mut self, id: u64) -> bool {
        self.id = id;
        true
    }
    fn get_id(&mut self) -> u64 {
        self.id
    }

    fn get_job_id(&mut self) -> Option<String> {
        match self.params.get(1) {
            Some(s) => Some(s.to_string()),
            None => todo!(),
        }
    }

    fn get_wallet(&mut self) -> Option<String> {
        match self.params.get(0) {
            Some(s) => Some(s.to_string()),
            None => todo!(),
        }
    }

    fn get_worker_name(&mut self) -> String {
        "Default".to_string()
    }

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
        true
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientWithWorkerName {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
    pub worker: String,
}

impl ClientRpc for ClientWithWorkerName {
    fn set_id(&mut self, id: u64) -> bool {
        self.id = id;
        true
    }

    fn get_id(&mut self) -> u64 {
        self.id
    }

    fn get_job_id(&mut self) -> Option<String> {
        match self.params.get(1) {
            Some(s) => Some(s.to_string()),
            None => todo!(),
        }
    }

    fn get_wallet(&mut self) -> Option<String> {
        match self.params.get(0) {
            Some(s) => Some(s.to_string()),
            None => todo!(),
        }
    }

    fn get_worker_name(&mut self) -> String {
        self.worker.clone()
    }

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
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientGetWork {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSubmitHashrate {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSideJob {
    pub id: u64,
    pub jsonrpc: String,
    pub result: Vec<String>,
}
impl ServerRpc for ServerSideJob {
    fn set_result(&mut self, res: Vec<std::string::String>) -> bool {
        self.result = res;

        true
    }
    fn set_diff(&mut self, _diff: String) -> bool {
        true
    }

    fn get_diff(&self) -> u64 {
        let job_diff = match self.result.get(3) {
            Some(diff) => {
                if diff.contains("0x") {
                    if let Some(h) = hex_to_int(&diff[2..diff.len()]) {
                        h as u64
                    } else if let Some(h) = hex_to_int(&diff[..]) {
                        h as u64
                    } else {
                        0
                    }
                } else {
                    if let Some(h) = hex_to_int(&diff[..]) {
                        h as u64
                    } else {
                        0
                    }
                }
            }
            None => 0,
        };

        job_diff
    }

    fn get_job_id(&self) -> Option<String> {
        match self.result.get(0) {
            Some(s) => Some(s.to_string()),
            None => todo!(),
        }
    }
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: u64,
    pub result: Vec<String>,
}

impl ServerRpc for Server {
    fn set_result(&mut self, res: Vec<std::string::String>) -> bool {
        self.result = res;
        true
    }

    fn set_diff(&mut self, _diff: String) -> bool {
        true
    }

    fn get_diff(&self) -> u64 {
        let job_diff = match self.result.get(3) {
            Some(diff) => {
                if diff.contains("0x") {
                    if let Some(h) = hex_to_int(&diff[2..diff.len()]) {
                        h as u64
                    } else if let Some(h) = hex_to_int(&diff[..]) {
                        h as u64
                    } else {
                        log::error!("收到任务JobId 字段不存在{:?}", self);
                        0
                    }
                } else {
                    if let Some(h) = hex_to_int(&diff[..]) {
                        h as u64
                    } else {
                        log::error!("收到任务JobId 字段不存在{:?}", self);
                        0
                    }
                }
            }
            None => {
                log::error!("收到任务JobId 字段不存在{:?}", self);
                0
            }
        };

        job_diff
    }

    fn get_job_id(&self) -> Option<String> {
        match self.result.get(0) {
            Some(s) => Some(s.to_string()),
            None => todo!(),
        }
    }
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRoot {
    pub id: u64,
    pub result: bool,
    pub error: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerError {
    pub id: u64,
    pub result: bool,
    pub error: EthError,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthError {
    pub code: u64,
    pub message: String,
}

impl std::fmt::Display for EthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "code: {}  msg : {}", self.code, self.message)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerJobsWithHeight {
    pub id: u64,
    pub result: Vec<String>,
    pub jsonrpc: String,
    pub height: u64,
}

impl ServerRpc for ServerJobsWithHeight {
    fn set_result(&mut self, res: Vec<std::string::String>) -> bool {
        self.result = res;

        true
    }
    fn set_diff(&mut self, _diff: String) -> bool {
        true
    }

    fn get_diff(&self) -> u64 {
        self.height
    }

    fn get_job_id(&self) -> Option<String> {
        match self.result.get(0) {
            Some(s) => Some(s.to_string()),
            None => todo!(),
        }
    }
}
//币印 {"id":0,"jsonrpc":"2.0","result":["0x0d08e3f8adaf9b1cf365c3f380f1a0fa4b7dda99d12bb59d9ee8b10a1a1d8b91","0x1bccaca36bfde6e5a161cf470cbf74830d92e1013ee417c3e7c757acd34d8e08","0x000000007fffffffffffffffffffffffffffffffffffffffffffffffffffffff","00"], "height":13834471}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerId1 {
    pub id: u64,
    pub result: bool,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerId {
    pub id: u64,
    pub jsonrpc: String,
    pub result: bool,
}
//{"id":4,"jsonrpc":"2.0","result":true}

//{"id":197,"result":false,"error":[21,"Job not found (=stale)",null]}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRootError {
    pub id: i64,
    pub result: bool,
    pub error: (i64, String, Value),
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerRootErrorValue {
    pub id: i64,
    pub result: Value,
    pub error: String,
}

pub fn handle_error(worker_id: u64, buf: &[u8]) {
    if let Ok(rpc) = serde_json::from_slice::<crate::protocol::rpc::eth::ServerError>(&buf) {
        log::warn!("抽水矿机 {} Share Reject: {}", worker_id, rpc.error);
    } else if let Ok(_rpc) = serde_json::from_slice::<crate::protocol::rpc::eth::ServerRoot>(&buf) {
        //log::warn!("抽水矿机 {} Share Reject: {}", worker_id, rpc.error);
    } else if let Ok(rpc) =
        serde_json::from_slice::<crate::protocol::rpc::eth::ServerRootError>(&buf)
    {
        log::warn!("抽水矿机 {} Share Reject: {}", worker_id, rpc.error.1);
    } else {
        log::warn!("抽水矿机 {} Share Reject: {:?}", worker_id, buf);
    }
}

pub fn handle_error_for_worker(worker_name: &String, buf: &[u8]) {
    if let Ok(rpc) = serde_json::from_slice::<crate::protocol::rpc::eth::ServerError>(&buf) {
        log::warn!("矿机 {} Share Reject: {}", worker_name, rpc.error);
    } else if let Ok(_rpc) = serde_json::from_slice::<crate::protocol::rpc::eth::ServerRoot>(&buf) {
        //log::warn!("矿机 {} Share Reject: {}", worker_name, rpc.error);
    } else if let Ok(rpc) =
        serde_json::from_slice::<crate::protocol::rpc::eth::ServerRootError>(&buf)
    {
        log::warn!("矿机 {} Share Reject: {}", worker_name, rpc.error.1);
    } else {
        log::warn!("矿机 {} Share Reject: {:?}", worker_name, buf);
    }
}
