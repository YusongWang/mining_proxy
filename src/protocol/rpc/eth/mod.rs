use serde::{Deserialize, Serialize};

//{\"id\":1,\"method\":\"eth_submitLogin\",\"params\":[\"0x98be5c44d574b96b320dffb0ccff116bda433b8e\",\"x\"],\"worker\":\"P0002\"}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
    //pub worker: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientWithWorkerName {
    pub id: u64,
    pub method: String,
    pub params: Vec<String>,
    pub worker: String,
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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: u64,
    pub result: Vec<String>,
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
pub struct ServerJobsWichHeigh {
    pub id: u64,
    pub result: Vec<String>,
    pub jsonrpc: String,
    pub height: u64,
}

//币印 {"id":0,"jsonrpc":"2.0","result":["0x0d08e3f8adaf9b1cf365c3f380f1a0fa4b7dda99d12bb59d9ee8b10a1a1d8b91","0x1bccaca36bfde6e5a161cf470cbf74830d92e1013ee417c3e7c757acd34d8e08","0x000000007fffffffffffffffffffffffffffffffffffffffffffffffffffffff","00"], "height":13834471}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerId1 {
    pub id: u64,
    pub result: bool,
}
