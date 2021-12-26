use serde::{Deserialize, Serialize};

pub trait ServerRpc {
    fn set_result(&mut self, res: Vec<std::string::String>) -> bool;
    fn set_diff(&mut self, diff: String) -> bool;
}

pub trait ClientRpc {
    fn set_id(&mut self, id: u64) -> bool;
    fn get_id(&mut self) -> u64;

    fn get_job_id(&mut self) -> Option<String>;
    fn get_wallet(&mut self) -> Option<String>;

    fn get_worker_name(&mut self) -> String;
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
    fn set_diff(&mut self, diff: String) -> bool {
        true
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
    fn set_diff(&mut self, diff: String) -> bool {
        true
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
    fn set_diff(&mut self, diff: String) -> bool {
        true
    }
}
//币印 {"id":0,"jsonrpc":"2.0","result":["0x0d08e3f8adaf9b1cf365c3f380f1a0fa4b7dda99d12bb59d9ee8b10a1a1d8b91","0x1bccaca36bfde6e5a161cf470cbf74830d92e1013ee417c3e7c757acd34d8e08","0x000000007fffffffffffffffffffffffffffffffffffffffffffffffffffffff","00"], "height":13834471}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerId1 {
    pub id: u64,
    pub result: bool,
}
