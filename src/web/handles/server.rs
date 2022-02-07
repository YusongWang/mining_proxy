use actix_web_grants::proc_macro::has_permissions;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
};

use clap::crate_version;

use actix_web::{get, post, web, Responder};
use human_bytes::human_bytes;
use serde::{Deserialize, Serialize};

use crate::{
    state::Worker,
    util::{config::Settings, time_to_string},
    web::{data::*, AppState, OnlineWorker},
};

#[post("/crate/app")]
#[has_permissions("ROLE_ADMIN")]
pub async fn crate_app(
    req: web::Json<CreateRequest>, app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    //dbg!(req);
    let mut config = Settings::default();
    if req.name == "" {
        return Ok(web::Json(Response::<String> {
            code: 40000,
            message: "中转名称必须填写".into(),
            data: String::default(),
        }));
    }

    if req.tcp_port == 0 && req.ssl_port == 0 && req.encrypt_port == 0 {
        return Ok(web::Json(Response::<String> {
            code: 40000,
            message: "未开启端口。请至少开启一个端口".into(),
            data: String::default(),
        }));
    }

    if req.pool_address.is_empty() {
        //println!("中转矿池必须填写");
        return Ok(web::Json(Response::<String> {
            code: 40000,
            message: "中转矿池必须填写".into(),
            data: String::default(),
        }));
    }

    if req.share != 0 {
        if req.share_address.is_empty() {
            //println!("抽水矿池必须填写");
            return Ok(web::Json(Response::<String> {
                code: 40000,
                message: "抽水矿池必须填写".into(),
                data: String::default(),
            }));
        }

        if req.share_wallet.is_empty() {
            //println!("抽水钱包必须填写");
            return Ok(web::Json(Response::<String> {
                code: 40000,
                message: "抽水钱包必须填写".into(),
                data: String::default(),
            }));
        }

        if req.share_rate <= 0.0 {
            //println!("抽水比例必须填写");
            return Ok(web::Json(Response::<String> {
                code: 40000,
                message: "抽水比例必须填写".into(),
                data: String::default(),
            }));
        }
    }

    config.share_name = req.name.clone();
    config.log_level = 1;
    config.log_path = "".into();
    config.name = req.name.clone();
    config.pool_address = vec![req.pool_address.clone()];
    config.share_address = vec![req.share_address.clone()];
    config.tcp_port = req.tcp_port;
    config.ssl_port = req.ssl_port;
    config.encrypt_port = req.encrypt_port;
    config.share = req.share;
    config.share_rate = req.share_rate as f32 / 100.0;
    config.share_wallet = req.share_wallet.clone();
    config.key = req.key.clone();
    config.iv = req.iv.clone();

    match config.check() {
        Ok(_) => {}
        Err(err) => {
            log::error!("配置错误 {}", err);
            return Ok(web::Json(Response::<String> {
                code: 40000,
                message: format!("配置错误 {}", err),
                data: String::default(),
            }));
            //std::process::exit(1);
        }
    };

    use std::fs::File;

    let mut cfgs = match OpenOptions::new()
        //.append(false)
        .write(true)
        .read(true)
        //.create(true)
        //.truncate(true)
        .open("configs.yaml")
    {
        Ok(f) => f,
        Err(_) => match File::create("configs.yaml") {
            Ok(t) => t,
            Err(e) => std::panic::panic_any(e),
        },
    };

    let mut configs = String::new();
    match cfgs.read_to_string(&mut configs) {
        Ok(_) => {
            let mut configs: Vec<Settings> =
                match serde_yaml::from_str(&configs) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("{}", e);
                        vec![]
                    }
                };

            // 去重
            for c in &configs {
                if config.name == c.name {
                    return Ok(web::Json(Response::<String> {
                        code: 40000,
                        message: format!("配置错误 服务器名: {} 已经存在，请修改后重新添加。",config.name),
                        data: String::default(),
                    }));
                }
            }
            configs.push(config.clone());
            match serde_yaml::to_string(&configs) {
                Ok(mut c_str) => {
                    c_str = c_str[4..c_str.len()].to_string();
                    drop(cfgs);
                    std::fs::remove_file("configs.yaml")?;
                    let mut cfgs = match OpenOptions::new()
                        //.append(false)
                        .write(true)
                        .read(true)
                        //.create(true)
                        //.truncate(true)
                        .open("configs.yaml")
                    {
                        Ok(f) => f,
                        Err(_) => match File::create("configs.yaml") {
                            Ok(t) => t,
                            Err(e) => std::panic::panic_any(e),
                        },
                    };

                    match cfgs.write_all(c_str.as_bytes()) {
                        Ok(()) => {}
                        Err(e) => {
                            return Ok(web::Json(Response::<String> {
                                code: 40000,
                                message: e.to_string(),
                                data: String::default(),
                            }))
                        }
                    }
                }
                Err(e) => {
                    return Ok(web::Json(Response::<String> {
                        code: 40000,
                        message: e.to_string(),
                        data: String::default(),
                    }))
                }
            };

            match crate::util::run_server(&config) {
                Ok(child) => {
                    let online = OnlineWorker {
                        child,
                        config: config.clone(),
                        workers: vec![],
                        online: 0,
                    };
                    app.lock().unwrap().insert(config.name, online);
                }
                Err(e) => {
                    return Ok(web::Json(Response::<String> {
                        code: 40000,
                        message: e.to_string(),
                        data: String::default(),
                    }))
                }
            }

            return Ok(web::Json(Response::<String> {
                code: 20000,
                message: "".into(),
                data: String::default(),
            }));
        }
        Err(_) => {
            let mut configs: Vec<Settings> = vec![];

            configs.push(config.clone());

            match serde_yaml::to_string(&configs) {
                Ok(mut c_str) => {
                    c_str = c_str[4..c_str.len()].to_string();
                    match cfgs.write_all(c_str.as_bytes()) {
                        Ok(()) => {}
                        Err(e) => {
                            return Ok(web::Json(Response::<String> {
                                code: 40000,
                                message: e.to_string(),
                                data: String::default(),
                            }))
                        }
                    }
                }
                Err(e) => {
                    return Ok(web::Json(Response::<String> {
                        code: 40000,
                        message: e.to_string(),
                        data: String::default(),
                    }))
                }
            };

            match crate::util::run_server(&config) {
                Ok(child) => {
                    let online = OnlineWorker {
                        child,
                        config: config.clone(),
                        workers: vec![],
                        online: 0,
                    };
                    app.lock().unwrap().insert(config.name, online);
                }
                Err(e) => {
                    return Ok(web::Json(Response::<String> {
                        code: 40000,
                        message: e.to_string(),
                        data: String::default(),
                    }))
                }
            }

            return Ok(web::Json(Response::<String> {
                code: 20000,
                message: "".into(),
                data: String::default(),
            }));
        }
    };
}

#[get("/user/server_list")]
#[has_permissions("ROLE_ADMIN")]
async fn server_list(
    app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let mut v = vec![];
    {
        let proxy_server = app.lock().unwrap();
        for (s, _) in &*proxy_server {
            v.push(s.to_string());
        }
    }

    Ok(web::Json(Response::<Vec<String>> {
        code: 20000,
        message: "".into(),
        data: v,
    }))
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ResWorker {
    pub worker_name: String,
    pub worker_wallet: String,
    pub hash: String,
    pub share_index: u64,
    pub accept_index: u64,
    pub fee_accept_index: u64,
    pub invalid_index: u64,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct OnlineWorkerResult {
    pub workers: Vec<ResWorker>,
    pub online: u32,
    pub online_time: String,
    pub config: Settings,
    pub fee_hash: String,
    pub total_hash: String,
    pub accept_index: u64,
    pub share_index: u64,
    pub reject_index: u64,
    pub fee_accept_index: u64,
    pub fee_share_index: u64,
    pub fee_reject_index: u64,
    pub rate: f64,
    pub share_rate: f64,
}

// 展示选中的数据信息。以json格式返回
#[get("/user/server/{name}")]
async fn server(
    proxy_server_name: web::Path<String>, app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let mut total_hash: f64 = 0.0;

    let mut res: OnlineWorkerResult = OnlineWorkerResult::default();
    {
        let proxy_server = app.lock().unwrap();
        let mut online = 0;
        let mut accept_index: u64 = 0;
        let mut share_index: u64 = 0;
        let mut reject_index: u64 = 0;
        let mut fee_accept_index: u64 = 0;
        let mut fee_share_index: u64 = 0;
        let mut fee_reject_index: u64 = 0;

        for (name, server) in &*proxy_server {
            if *name == proxy_server_name.to_string() {
                for r in &server.workers {
                    if r.is_online() {
                        online += 1;
                        total_hash += r.hash as f64;
                        res.workers.push(ResWorker {
                            worker_name: r.worker_name.clone(),
                            worker_wallet: r.worker_wallet.clone(),
                            hash: human_bytes(r.hash as f64),
                            share_index: r.share_index,
                            accept_index: r.accept_index,
                            invalid_index: r.invalid_index,
                            fee_accept_index: r.fee_accept_index,
                        });

                        share_index += r.share_index;
                        accept_index += r.accept_index;
                        reject_index += r.invalid_index;
                        fee_accept_index += r.fee_share_index;
                        fee_share_index += r.fee_accept_index;
                        fee_reject_index += r.fee_invalid_index;
                    }
                }
                res.config = server.config.clone();
            }
        }

        res.online = online;
        if res.online >= 1 {
            res.share_index = share_index;
            res.accept_index = accept_index;
            res.reject_index = reject_index;
            res.fee_accept_index = fee_accept_index;
            res.fee_share_index = fee_share_index;
            res.fee_reject_index = fee_reject_index;

            res.rate = floor(
                res.accept_index as f64 / res.share_index as f64 * 100.0,
                2,
            );
            res.share_rate = floor(
                res.fee_accept_index as f64 / res.accept_index as f64 * 100.0,
                2,
            );
        }

        res.fee_hash =
            human_bytes(total_hash as f64 * res.config.share_rate as f64);
        res.total_hash = human_bytes(total_hash as f64);
    }

    //1. 基本配置文件信息 .
    //2. 抽水旷工信息     .
    //3. 当前在线矿机总数 .

    Ok(web::Json(Response::<OnlineWorkerResult> {
        code: 20000,
        message: "".into(),
        data: res,
    }))
}

pub fn floor(value: f64, scale: i8) -> f64 {
    let multiplier = 10f64.powi(scale as i32) as f64;
    (value * multiplier).floor() / multiplier
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct DashboardResult {
    pub proxy_num: i32,
    pub online: u32,
    pub fee_hash: String,
    pub total_hash: String,
    pub accept_index: u64,
    pub share_index: u64,
    pub reject_index: u64,
    pub fee_accept_index: u64,
    pub fee_share_index: u64,
    pub fee_reject_index: u64,
    pub rate: f64,       //总代理算力
    pub share_rate: f64, //抽水算力
    pub version: String,
    pub develop_worker_name: String,
    pub online_time: String,
}

// 展示选中的数据信息。以json格式返回
#[post("/user/dashboard")]
async fn dashboard(
    app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let mut total_hash: f64 = 0.0;
    let mut fee_hash: f64 = 0.0;
    let mut res: DashboardResult = DashboardResult::default();
    {
        let proxy_server = app.lock().unwrap();
        let mut online = 0;
        let mut accept_index: u64 = 0;
        let mut share_index: u64 = 0;
        let mut reject_index: u64 = 0;
        let mut fee_accept_index: u64 = 0;
        let mut fee_share_index: u64 = 0;
        let mut fee_reject_index: u64 = 0;

        for (_, other_server) in &*proxy_server {
            for r in &other_server.workers {
                if r.is_online() {
                    online += 1;
                    total_hash += r.hash as f64;
                    share_index += r.share_index;
                    accept_index += r.accept_index;
                    reject_index += r.invalid_index;
                    fee_accept_index += r.fee_share_index;
                    fee_share_index += r.fee_accept_index;
                    fee_reject_index += r.fee_invalid_index;
                }
            }

            fee_hash +=
                total_hash as f64 * other_server.config.share_rate as f64;
        }

        res.share_index += share_index;
        res.accept_index += accept_index;
        res.reject_index += reject_index;
        res.fee_accept_index += fee_accept_index;
        res.fee_share_index += fee_share_index;
        res.fee_reject_index += fee_reject_index;

        res.proxy_num = proxy_server.len() as i32;
        res.online = online;
    }

    res.fee_hash = human_bytes(fee_hash as f64);
    res.total_hash = human_bytes(total_hash as f64);
    if res.accept_index > 0 {
        res.rate =
            floor(res.accept_index as f64 / res.share_index as f64 * 100.0, 2);
        res.share_rate = floor(
            res.fee_accept_index as f64 / res.accept_index as f64 * 100.0,
            2,
        );
    } else {
        res.rate = 0.0;
        res.share_rate = 0.0;
    }

    res.online_time = time_to_string(crate::RUNTIME.elapsed().as_secs());
    res.develop_worker_name = crate::DEVELOP_WORKER_NAME.clone();
    res.version = crate_version!().to_string();

    Ok(web::Json(Response::<DashboardResult> {
        code: 20000,
        message: "".into(),
        data: res,
    }))
}
