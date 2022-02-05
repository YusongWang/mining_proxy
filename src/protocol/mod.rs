pub mod eth_stratum;
pub mod ethjson;
pub mod rpc;
pub mod stratum;

use num_enum::IntoPrimitive;
use serde::{Deserialize, Serialize};

pub const CLIENT_LOGIN: u64 = 1001;
pub const CLIENT_GETWORK: u64 = 1005;
pub const CLIENT_SUBHASHRATE: u64 = 1006;
pub const CLIENT_SUBMITWORK: u64 = 1000;
pub const SUBSCRIBE: u64 = 10002;

#[derive(
    Debug, Eq, Clone, IntoPrimitive, PartialEq, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum PROTOCOL {
    STRATUM,
    ETH,
    NICEHASHSTRATUM,
    KNOWN,
}
