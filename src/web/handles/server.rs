use actix_web_grants::proc_macro::has_permissions;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
};

use actix_web::{get, post, web, Responder};
use serde::{Deserialize, Serialize};
use human_bytes::human_bytes;


use crate::{
    state::Worker,
    util::config::Settings,
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

        if req.share_rate == 0 {
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

    // let exe_path =
    // std::env::current_dir().expect("获取当前可执行程序路径错误");
    // let exe_path = exe_path.join("configs.yaml");
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

            dbg!(configs.clone());
            configs.push(config.clone());

            dbg!(configs.clone());
            match serde_yaml::to_string(&configs) {
                Ok(mut c_str) => {
                    dbg!(c_str.clone());
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
            dbg!(configs.clone());
            configs.push(config.clone());

            dbg!(configs.clone());
            match serde_yaml::to_string(&configs) {
                Ok(mut c_str) => {
                    dbg!(c_str.clone());
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
            log::info!("server {} ", s);
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
    pub invalid_index: u64,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct OnlineWorkerResult {
    pub workers: Vec<ResWorker>,
    pub online: u32,
    pub config: Settings,
}

// 展示选中的数据信息。以json格式返回
#[get("/user/server/{name}")]
async fn server(
    proxy_server_name: web::Path<String>, app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    log::debug!("{}", proxy_server_name);

    let mut res: OnlineWorkerResult = OnlineWorkerResult::default();
    {
        let proxy_server = app.lock().unwrap();
        for (name, server) in &*proxy_server {
            log::info!("server {} ", name);
            if *name == proxy_server_name.to_string() {
                //config.kill().await?;
                //server.child.kill().await?;

                for r in &server.workers {
                    res.workers.push(ResWorker {
                        worker_name: r.worker_name.clone(),
                        worker_wallet: r.worker_wallet.clone(),
                        hash: human_bytes(r.hash as f64),
                        share_index: r.share_index,
                        accept_index: r.accept_index,
                        invalid_index: r.invalid_index,
                    });
                }

                res.online = server.online.clone();
                res.config = server.config.clone();
            }
        }
        res.online = proxy_server.len() as u32;
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
