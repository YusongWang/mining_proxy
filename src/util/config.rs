use std::env;

use anyhow::Result;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

use super::get_develop_fee;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub name: String,
    pub log_level: u32,
    pub log_path: String,
    pub ssl_port: u32,
    pub tcp_port: u32,
    pub encrypt_port: u32,
    pub pool_ssl_address: Vec<String>,
    pub pool_tcp_address: Vec<String>,
    pub share_tcp_address: Vec<String>,
    pub share_ssl_address: Vec<String>,
    pub share_wallet: String,
    pub share_name: String,
    pub share_rate: f32,
    pub share: u32,
    pub share_alg: u32,
    pub p12_path: String,
    pub p12_pass: String,
    pub key: String,
    pub iv: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_level: 6,
            log_path: "".into(),
            pool_ssl_address: Vec::new(),
            pool_tcp_address: Vec::new(),
            share_tcp_address: Vec::new(),
            share_ssl_address: Vec::new(),
            share_wallet: "".into(),
            share_rate: 0.0,
            ssl_port: 8443,
            tcp_port: 14444,
            encrypt_port: 14444,
            p12_path: "./identity.p12".into(),
            p12_pass: "mypass".into(),
            share: 0,
            share_name: "".into(),
            name: "proxy".into(),
            share_alg: 0,
            key: "0000000000000000000000".into(),
            iv: "123456".into(),
        }
    }
}

impl Settings {
    pub fn new(file_path: &str, with_file: bool) -> Result<Self, ConfigError> {
        let mut s = Config::default();

        if with_file {
            s.merge(File::with_name(file_path).required(false))?;
        }
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

        match env::var("PROXY_POOL_TCP_ADDRESS") {
            Ok(tcp_address) => {
                let arr: Vec<&str> = tcp_address.split(',').collect();
                s.set("pool_tcp_address", arr)?;
            }
            Err(_) => {}
        }

        match env::var("PROXY_POOL_SSL_ADDRESS") {
            Ok(tcp_address) => {
                let arr: Vec<&str> = tcp_address.split(',').collect();
                s.set("pool_ssl_address", arr)?;
            }
            Err(_) => {}
        }

        match env::var("PROXY_SHARE_TCP_ADDRESS") {
            Ok(tcp_address) => {
                let arr: Vec<&str> = tcp_address.split(',').collect();
                s.set("share_tcp_address", arr)?;
            }
            Err(_) => {}
        }

        match env::var("PROXY_SHARE_SSL_ADDRESS") {
            Ok(tcp_address) => {
                let arr: Vec<&str> = tcp_address.split(',').collect();
                s.set("share_ssl_address", arr)?;
            }
            Err(_) => {}
        }
        s.try_into()
    }

    pub fn get_fee(&self) -> f64 {
        let develop_fee = get_develop_fee(self.share_rate.into());

        let share_fee = self.share_rate;

        develop_fee + share_fee as f64
    }

    pub fn get_share_name(&self) -> Result<String> {
        let mut hostname = self.share_name.clone();
        if hostname.is_empty() {
            let name = hostname::get()?;
            if name.is_empty() {
                hostname = "proxy_wallet_mine".into();
            } else {
                hostname = hostname + name.to_str().unwrap();
            }
        }
        Ok(hostname)
    }
}
