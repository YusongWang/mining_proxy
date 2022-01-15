extern crate vergen;

use std::{env, fs::File, io::Write, path::PathBuf};

use vergen::*;
fn gen_agent_wallet(agent_wallet: String) -> String {
    let mut now_fn = String::from("/// Generate wallet \n");
    now_fn.push_str("pub fn agent() -> &'static str {\n");
    now_fn.push_str("    \"");
    now_fn.push_str(&agent_wallet[..]);
    now_fn.push_str("\"\n");
    now_fn.push_str("}\n\n");

    now_fn
}

fn main() {
    vergen(SHORT_SHA | COMMIT_DATE).unwrap();
    match env::var("AGNET") {
        Ok(v) => {
            let out = env::var("OUT_DIR").unwrap();
            let dst = PathBuf::from(out);
            let mut f = File::create(&dst.join("agent.rs")).unwrap();
            f.write_all(gen_agent_wallet(v).as_bytes()).unwrap();
        }
        Err(_e) => {}
    }

    if let Ok(v) = env::var("DEP_OPENSSL_VERSION_NUMBER") {
        let version = u64::from_str_radix(&v, 16).unwrap();

        if version >= 0x1_01_01_00_0 {
            println!("cargo:rustc-cfg=openssl111");
        }
    }
}
