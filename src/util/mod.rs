pub mod config;
pub mod logger;

mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

extern crate clap;

use anyhow::Result;
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};

use self::config::Settings;

pub async fn get_app_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(format!(
        "{}, 版本: {} commit: {} {}",
        crate_name!(),
        crate_version!(),
        version::commit_date(),
        version::short_sha()
    ))
    .version(crate_version!())
    .author(crate_authors!("\n"))
    .about(crate_description!())
    .arg(
        Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Sets a custom config file")
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
            parse_hex_digit(c).and_then(|n| acc.map(|acc| acc + n * base.pow(pos as u32)))
        })
}

pub fn calc_hash_rate(my_hash_rate: u64, share_rate: f32) -> u64 {
    ((my_hash_rate) as f32 * share_rate) as u64
}

// 根据抽水率计算启动多少个线程
pub fn clac_phread_num(rate: f64) -> u64 {
    (rate * 1000.0) as u64
}

#[test]
fn test_clac_phread_num() {
    assert_eq!(clac_phread_num(0.005), 5);
    assert_eq!(clac_phread_num(0.08), 80);
}

// 根据抽水率计算启动多少个线程
pub fn clac_phread_num_for_real(rate: f64) -> u64 {
    //let mut num = 5;

    //if rate <= 0.01 {
    //        num = 5;
    // } else if rate <= 0.05 {
    //     num = 2;
    // } else if rate <= 0.1 {
    //     num = 1;
    // }

    let mut phread_num = clac_phread_num(rate);
    if phread_num <= 0 {
        phread_num = 1;
    }

    extern crate num_cpus;
    let cpu_nums = num_cpus::get();
    if cpu_nums > 1 {
        phread_num *= 2;
    }
    // *CPU核心数。
    phread_num
}

pub fn is_fee(idx: u64, fee: f64) -> bool {
    let rate = clac_phread_num(fee);

    idx % (1000 / rate) == 0
}

#[test]
fn test_is_fee() {
    assert_eq!(is_fee(200, 0.005), true);
    assert_ne!(is_fee(201, 0.005), true);
    assert_eq!(is_fee(200, 0.1), true);
    let mut idx = 0;
    for i in 0..100 {
        if is_fee(i, 0.1) {
            idx += 1;
        }
    }
    assert_eq!(idx, 10);

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
}

pub fn is_fee_random(mut fee: f64) -> bool {
    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
    let secret_number = rand::Rng::gen_range(&mut rng, 1..1000);

    if fee <= 0.000 {
        fee = 0.001;
    }

    let max = (1000.0 * fee) as u32;
    let max = 1000 - max;
    match secret_number.cmp(&max) {
        std::cmp::Ordering::Less => {
            return false;
        }
        _ => {
            return true;
        }
    }
}

pub fn fee(idx: u64, config: &Settings, fee: f64) -> bool {
    if config.share_alg == 1 {
        return is_fee(idx, fee);
    } else {
        return is_fee_random(fee);
    }
}

#[test]
fn test_fee() {
    let mut config = Settings::default();
    config.share_alg = 1;
    let mut i = 0;
    for idx in 0..1000 {
        if fee(idx, &config, 0.005) {
            i += 1;
        }
    }

    assert_eq!(i, 5);

    let mut i = 0;
    for idx in 0..1000 {
        if fee(idx, &config, 0.005) {
            i += 1;
        }
    }

    //assert_eq!(i,5);
}
#[test]
fn test_is_fee_random() {
    let mut i = 0;
    for _ in 0..1000 {
        if is_fee_random(0.005) {
            i += 1;
        }
    }
    assert_eq!(i, 5);
}
