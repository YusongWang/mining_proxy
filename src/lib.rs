#![allow(dead_code)]

const SPLIT: u8 = b'\n';
const WALLET: &'static str = "0x98be5c44d574b96b320dffb0ccff116bda433b8e";

pub mod agent;
pub mod client;
pub mod protocol;
//pub mod server;
pub mod state;
pub mod util;

pub mod web;
