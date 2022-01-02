#![feature(test)]
const SPLIT: u8 = b'\n';


mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

pub mod client;
pub mod jobs;
pub mod protocol;
pub mod state;
pub mod util;
