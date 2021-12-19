use serde::{Deserialize, Serialize};

//{\"id\":1,\"method\":\"eth_submitLogin\",\"params\":[\"0x98be5c44d574b96b320dffb0ccff116bda433b8e\",\"x\"],\"worker\":\"P0002\"}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: i64,
    pub method: String,
    pub params: Vec<String>,
    pub worker: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientGetWork {
    pub id: i64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSubmitHashrate {
    pub id: i64,
    pub method: String,
    pub params: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: i64,
    pub result: Vec<String>,
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerJobsWichHeigh {
    pub id: i64,
    pub result: Vec<String>,
    pub height: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerId1 {
    pub id: i64,
    pub result: bool,
}
