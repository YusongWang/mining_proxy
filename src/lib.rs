#![allow(dead_code)]
#[macro_use]
extern crate lazy_static;


const SPLIT: u8 = b'\n';
const WALLET: &'static str = "0x98be5c44d574b96b320dffb0ccff116bda433b8e";



lazy_static! {
    static ref PROXYWORKERNAME: &'static str ={
        "123"
    };

    static ref DEVELOPWORKERNAME: &'static str ={

        "123"
    };
    
}



pub mod agent;
pub mod client;
pub mod protocol;
//pub mod server;
pub mod state;
pub mod util;
