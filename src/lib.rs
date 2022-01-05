const SPLIT: u8 = b'\n';
#[macro_use]
extern crate diesel;
pub mod web;
pub mod client;
pub mod jobs;
pub mod protocol;
pub mod state;
pub mod util;
pub mod agent;