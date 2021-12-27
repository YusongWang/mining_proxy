use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthServerRootObject {
    pub id: u64,
    pub jsonrpc: String,
    pub result: Vec<String>,
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

// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct Server {
//     pub id: u64,
//     pub result: Vec<String>,
// }

// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ServerError {
//     pub id: u64,
//     pub jsonrpc: String,
//     pub error: String,
// }

// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct BinanceError {
//     pub code: u64,
//     pub message: String,
// }

// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ServerJobsWichHeigh {
//     pub id: u64,
//     pub result: Vec<String>,
//     pub jsonrpc: String,
//     pub height: u64,
// }

//币印 {"id":0,"jsonrpc":"2.0","result":["0x0d08e3f8adaf9b1cf365c3f380f1a0fa4b7dda99d12bb59d9ee8b10a1a1d8b91","0x1bccaca36bfde6e5a161cf470cbf74830d92e1013ee417c3e7c757acd34d8e08","0x000000007fffffffffffffffffffffffffffffffffffffffffffffffffffffff","00"], "height":13834471}

// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ServerId1 {
//     pub id: u64,
//     pub result: bool,
// }

// public class EthServerRootObject
// {
//     public int id { get; set; }
//     public string jsonrpc { get; set; }
//     public List<string> result { get; set; }
// }

// public class EthError
// {
//     public int code { get; set; }
//     public string message { get; set; }
// }

// public class EthServerRootObjectBool
// {
//     public int? id { get; set; }
//     public string jsonrpc { get; set; }
//     public bool? result { get; set; }
//     public EthError error { get; set; }
// }

// public class EthServerRootObjectError
// {
//     public int? id { get; set; }
//     public string jsonrpc { get; set; }
//     public bool? result { get; set; }
//     public string error { get; set; }
// }
