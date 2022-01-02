#![feature(test)]

mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

pub mod client;
pub mod jobs;
pub mod protocol;
pub mod state;
pub mod util;