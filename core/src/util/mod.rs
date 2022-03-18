pub mod config;
pub mod logger;

extern crate clap;

use anyhow::{bail, Result};
use clap::{
    crate_description, crate_name, crate_version, App, Arg, ArgMatches,
};

use self::config::Settings;

pub fn get_app_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, 版本: {}",
        crate_name!(),
        crate_version!() /* version::commit_date(),
                          * version::short_sha() */
    ))
    .version(crate_version!())
    //.author(crate_authors!("\n"))
    .about(crate_description!())
    .arg(
        Arg::with_name("server")
            .short("s")
            .long("server")
            .help("指定为server(代理)模式运行"),
    )
    .arg(
        Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("指定配置文件路径 默认 ./default.yaml")
            .takes_value(true),
    )
    .get_matches();
    Ok(matches)
}

fn parse_hex_digit(c: char) -> Option<i64> {
    match c {
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        'a' => Some(10),
        'b' => Some(11),
        'c' => Some(12),
        'd' => Some(13),
        'e' => Some(14),
        'f' => Some(15),
        _ => None,
    }
}

pub fn hex_to_int(string: &str) -> Option<i64> {
    let base: i64 = 16;

    string
        .chars()
        .rev()
        .enumerate()
        .fold(Some(0), |acc, (pos, c)| {
            parse_hex_digit(c)
                .and_then(|n| acc.map(|acc| acc + n * base.pow(pos as u32)))
        })
}

pub fn bytes_to_mb(hash: u64) -> u64 { hash / 1000 / 1000 }

pub fn calc_hash_rate(my_hash_rate: u64, share_rate: f32) -> u64 {
    ((my_hash_rate) as f32 * share_rate) as u64
}

// 根据抽水率计算启动多少个线程
pub fn clac_phread_num(rate: f64) -> u128 { (rate * 1000.0) as u128 }

#[test]
fn test_clac_phread_num() {
    assert_eq!(clac_phread_num(0.005), 5);
    assert_eq!(clac_phread_num(0.08), 80);
}

pub fn is_fee(idx: u128, fee: f64) -> bool { idx % (fee * 1000.0) as u128 == 0 }

#[test]
fn test_is_fee() {
    // assert_eq!(is_fee(200, 0.005), true);
    // assert_ne!(is_fee(201, 0.005), true);
    // assert_eq!(is_fee(200, 0.1), true);
    let mut idx = 0;
    for i in 0..1000 {
        if is_fee(i, 0.019) {
            println!("{}", i);
            idx += 1;
        }
    }

    assert_eq!(idx, 190);

    let mut idx = 0;
    for i in 0..1000 {
        if is_fee(i, 0.1) {
            idx += 1;
        }
    }
    assert_eq!(idx, 100);

    let mut idx = 0;
    for i in 0..10000 {
        if is_fee(i, 0.1) {
            idx += 1;
        }
    }
    assert_eq!(idx, 1000);

    let mut idx = 0;
    for i in 0..1000 {
        if is_fee(i, 0.22) {
            println!("{}", i);
            idx += 1;
        }
    }

    assert_eq!(idx, 220);

    let mut idx = 0;
    for i in 0..10000 {
        if is_fee(i, 0.5) {
            idx += 1;
        }
    }

    assert_eq!(idx, 5000);

    let mut idx = 0;
    for i in 0..10000 {
        if is_fee(i, 0.8) {
            idx += 1;
        }
    }

    assert_eq!(idx, 8000);
}

pub fn is_fee_random(mut fee: f64) -> bool {
    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
    let secret_number = rand::Rng::gen_range(&mut rng, 1..1000) as i32;

    if fee <= 0.000 {
        fee = 0.001;
    }

    let mut max = (1000.0 * fee) as i32;
    if (1000 - max) <= 0 {
        max = 0;
    } else {
        max = 1000 - max;
    }

    match secret_number.cmp(&max) {
        std::cmp::Ordering::Less => {
            return false;
        }
        _ => {
            return true;
        }
    }
}

// #[cfg(test)]
// mod tests {

//     extern crate test;
//     use super::*;
//     use test::Bencher;

//     #[bench]
//     fn bench_random_fee(b: &mut Bencher) {
//         b.iter(|| {
//             for _ in 0..10000 {
//                 is_fee_random(0.005);
//             }
//         })
//     }

//     #[bench]
//     fn bench_index_fee(b: &mut Bencher) {
//         b.iter(|| {
//             //let mut i = 0;
//             for _ in 0..10000 {
//                 is_fee(200, 0.005);
//             }
//         })
//     }
// }

pub fn fee(idx: u128, config: &Settings, fee: f64) -> bool {
    if config.share_alg == 1 {
        return is_fee(idx, fee);
    } else {
        return is_fee_random(fee);
    }
}

// #[test]
// fn test_fee() {
//     let mut config = Settings::default();
//     config.share_alg = 1;
//     let mut i = 0;
//     for idx in 0..1000 {
//         if fee(idx, &config, 0.005) {
//             i += 1;
//         }
//     }

//     assert_eq!(i, 5);
// }

// #[test]
// fn test_is_fee_random() {
//     let mut i = 0;
//     for _ in 0..1000 {
//         if is_fee_random(0.5) {
//             i += 1;
//         }
//     }
//     assert_eq!(i, 5);
// }

pub fn time_to_string(mut time: u64) -> String {
    let mut res = String::new();

    use chrono::{NaiveTime, Timelike};
    let day = time / 86_400;
    if day > 0 {
        let s = day.to_string() + "天";
        res += &s;
        time %= 86_400;
    }

    let t = match NaiveTime::from_num_seconds_from_midnight_opt(time as u32, 0)
    {
        Some(t) => t,
        None => return "格式化错误".into(),
    };

    if t.hour() > 0 {
        let s = t.hour().to_string() + "小时";
        res += &s;
    }

    if t.minute() > 0 {
        let s = t.minute().to_string() + "分钟";
        res += &s;
    }

    if t.second() > 0 {
        let s = t.second().to_string() + "秒";
        res += &s;
    }

    res += "前";

    return res;
}

// #[test]
// fn test_time_to_string() {
//     use chrono::{NaiveTime, Timelike};

//     let t = NaiveTime::from_num_seconds_from_midnight(1200, 0);

//     assert_eq!(t.hour(), 23);
//     assert_eq!(t.minute(), 56);
//     assert_eq!(t.second(), 4);
//     assert_eq!(t.nanosecond(), 12_345_678);
// }

cfg_if::cfg_if! {
    if #[cfg(feature = "agent")] {
        #[inline(always)]
        pub fn get_develop_fee(share_fee: f64,is_true:bool) -> f64 {
            0.001
        }
    } else {
        #[inline(always)]
        pub fn get_develop_fee(share_fee: f64,is_true:bool) -> f64 {
            if share_fee <= 0.01 {
                if is_true {
                    return 0.001;
                }
                return 0.001;
            } else if share_fee >= 0.03{
                return 0.003;
            } else {
                return 0.002;
            }
        }
    }
}

#[inline(always)]
pub fn get_agent_fee(share_fee: f64) -> f64 {
    if share_fee <= 0.05 {
        return 0.005;
    }
    share_fee / 10.0
}

//TODO 整理代码 删除无用代码。 目前折中防止报错
#[inline(always)]
pub fn get_eth_wallet() -> String { return "".into(); }

#[inline(always)]
pub fn get_etc_wallet() -> String { return "".into(); }

#[inline(always)]
pub fn get_cfx_wallet() -> String { return "".into(); }

pub fn run_server(config: &Settings) -> Result<tokio::process::Child> {
    let exe = std::env::current_exe().expect("无法获取当前可执行程序路径");
    let exe_path = std::env::current_dir().expect("获取当前可执行程序路径错误");

    let mut handle = tokio::process::Command::new(exe);

    let handle = handle
        .arg("--server")
        .env("PROXY_NAME", config.name.clone())
        .env("PROXY_LOG_LEVEL", config.log_level.to_string())
        .env("PROXY_TCP_PORT", config.tcp_port.to_string())
        .env("PROXY_SSL_PORT", config.ssl_port.to_string())
        .env("PROXY_ENCRYPT_PORT", config.encrypt_port.to_string())
        .env("PROXY_POOL_ADDRESS", config.pool_address[0].clone())
        .env("PROXY_SHARE_ADDRESS", config.share_address[0].clone())
        .env("PROXY_SHARE_RATE", config.share_rate.to_string())
        .env("PROXY_SHARE_WALLET", config.share_wallet.to_string())
        .env("PROXY_SHARE_ALG", config.share_alg.to_string())
        .env("PROXY_HASH_RATE", config.hash_rate.to_string())
        .env("PROXY_COIN", config.coin.to_string())
        .env("PROXY_SHARE_NAME", config.share_name.to_string())
        .env("PROXY_SHARE", config.share.to_string())
        .env(
            "PROXY_PEM_PATH",
            exe_path.to_str().expect("无法转换路径为字符串").to_string()
                + config.pem_path.as_str(),
        )
        .env(
            "PROXY_KEY_PATH",
            exe_path.to_str().expect("无法转换路径为字符串").to_string()
                + config.key_path.as_str(),
        );
    match handle.spawn() {
        Ok(t) => Ok(t),
        Err(e) => {
            bail!(e);
        }
    }
}

const SUFFIX: [&'static str; 9] =
    ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

pub fn human_bytes<T: Into<f64>>(size: T) -> String {
    let size = size.into();

    if size <= 0.0 {
        return "0 B".to_string();
    }

    let base = size.log10() / 1000_f64.log10();

    let mut result = format!("{:.1}", 1000_f64.powf(base - base.floor()),)
        .trim_end_matches(".0")
        .to_owned();

    // Add suffix
    result.push(' ');
    result.push_str(SUFFIX[base.floor() as usize]);

    result
}
