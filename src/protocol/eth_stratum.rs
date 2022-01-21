use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthLoginNotify {
    pub id: i64,
    pub jsonrpc: String,
    pub result: (Vec<String>, String),
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthSubscriptionNotify {
    pub id: i64,
    pub result: (Vec<String>, String),
    pub error: Value,
}
