pub mod ethjson;
pub mod eth_stratum;
pub mod rpc;
pub mod stratum;

pub const CLIENT_LOGIN: u64 = 1001;
pub const CLIENT_GETWORK: u64 = 1005;
pub const CLIENT_SUBHASHRATE: u64 = 1006;
pub const CLIENT_SUBMITWORK: u64 = 1000;
pub const SUBSCRIBE: u64 = 10002;

#[derive(PartialEq)]
pub enum PROTOCOL {
    STRATUM,
    ETH,
    NICEHASH,
    KNOWN,
}
