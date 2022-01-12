use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumRoot {
    pub id: i64,
    pub method: String,
    pub params: Vec<String>,
}





#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumResult{
    pub id: i64,
    pub result: (Vec<String>, String),
    pub error: Value,
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StraumResultWorkNotify {
    pub id: i64,
    pub method: String,
    pub params: (String, String, String, bool),
}



