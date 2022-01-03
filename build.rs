extern crate vergen;

use std::{env, path::PathBuf, fs::File, io::Write};

use vergen::*;
fn gen_agent_wallet() -> String {
    let mut now_fn = String::from("/// Generate wallet \n");
    now_fn.push_str("pub fn agent() -> &'static str {\n");
    let agent_wallet = env::var("AGNET").unwrap();
    now_fn.push_str("    \"");
    now_fn.push_str(&agent_wallet[..]);
    now_fn.push_str("\"\n");
    now_fn.push_str("}\n\n");

    now_fn
}

fn main() {
    vergen(SHORT_SHA | COMMIT_DATE).unwrap();
    match env::var("AGNET"){
        Ok(v) => {
            let out = env::var("OUT_DIR").unwrap();
            let dst = PathBuf::from(out);
            let mut f = File::create(&dst.join("agent.rs")).unwrap();
            f.write_all(gen_agent_wallet().as_bytes()).unwrap();
        },
        Err(_e) => {}
    }
}
