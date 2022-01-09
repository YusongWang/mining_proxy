use serde::{Deserialize, Serialize};

use crate::util::hex_to_int;

pub trait EthClientObject {
    fn set_id(&mut self, id: u64) -> bool;
    fn get_id(&mut self) -> u64;

    fn get_job_id(&mut self) -> Option<String>;
    fn get_wallet(&mut self) -> Option<String>;

    fn get_worker_name(&mut self) -> String;
    fn set_worker_name(&mut self, worker_name: &str) -> bool;

    fn get_submit_hashrate(&self) -> u64;
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

    fn set_worker_name(&mut self, _worker_name: &str) -> bool {
        true
    }
}

impl EthClientObject for EthClientWorkerObject {
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
