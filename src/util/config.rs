use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub name: String,
    pub log_level: u32,
    pub log_path: String,
    pub ssl_port: u32,
    pub tcp_port: u32,
    pub pool_ssl_address: Vec<String>,
    pub pool_tcp_address: Vec<String>,
    pub share_tcp_address: Vec<String>,
    pub share_wallet: String,
    pub share_name: String,
    pub share_rate: f32,
    pub share: u32,
    pub share_alg: u32,
    pub p12_path: String,
    pub p12_pass: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_level: 6,
            log_path: "".into(),
            pool_ssl_address: Vec::new(),
            pool_tcp_address: Vec::new(),
            share_tcp_address: Vec::new(),
            share_wallet: "".into(),
            share_rate: 0.0,
            ssl_port: 8443,
            tcp_port: 14444,
            p12_path: "./identity.p12".into(),
            p12_pass: "".into(),
            share: 0,
            share_name: "".into(),
            name: "proxy".into(),
            share_alg: 0,
        }
    }
}

impl Settings {
    pub fn new(file_path: &str) -> Result<Self, ConfigError> {
        let mut s = Config::default();

        // Start off by merging in the "default" configuration file
        s.merge(File::with_name(file_path).required(false))?;

        // Add in the current environment file
        // Default to 'development' env
        // Note that this file is _optional_
        // let env = env::var("RUN_MODE").unwrap_or_else(|_| "dev".into());
        // s.merge(File::with_name(&format!("config/{}", env)).required(false))?;

        // Add in a local configuration file
        // This file shouldn't be checked in to git
        //s.merge(File::with_name("config/local").required(false))?;

        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("PROXY"))?;

        // // You may also programmatically change settings
        // s.set("database.url", "postgres://")?;

        // // Now that we're done, let's access our configuration
        // println!("debug: {:?}", s.get_bool("debug"));
        // println!("database: {:?}", s.get::<String>("database.url"));

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_into()
    }
}
