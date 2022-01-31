use std::{
    fs::OpenOptions,
    io::{Read, Write},
};

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};

use crate::{util::config::Settings, web::AppState};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct TokenDataResponse {
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct InfoResponse {
    pub roles: Vec<String>,
    pub introduction: String,
    pub avatar: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Response<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct RegisterResponse {
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct LoginResponse {
    pub code: i32,
    pub data: TokenDataResponse,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Book {
    pub id: i32,
    pub name: String,
    pub operator: String,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CreateBookRequest {
    pub name: String,
}
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CreateBookResponse {
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SearchBookRequest {
    pub query: String,
}
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SearchBookResponse {
    pub books: Vec<Book>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct UpdateBookRequest {
    pub id: i32,
    pub name: String,
}
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct UpdateBookResponse {
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct DeleteBookRequest {
    pub id: i32,
}
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct DeleteBookResponse {
    pub success: bool,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct CreateRequest {
    pub name: String,
    pub tcp_port: u32,
    pub ssl_port: u32,
    pub encrypt_port: u32,
    pub share: u32,
    pub pool_address: String,
    pub share_address: String,
    pub share_rate: u32,
    pub share_wallet: String,
    pub key: String,
    pub iv: String,
}

// pub async fn login() -> Result<Json<LoginResponse>, StatusCode> {
//     let jwt_token = generate_jwt(&Claims {
//         username: "mining_proxy".to_string(),
//     })
//     .map_err(|err| {
//         log::error!("failed to generate jwt token: {}", err);
//         StatusCode::INTERNAL_SERVER_ERROR
//     })?;

//     Ok(Json(LoginResponse {
//         code: 20000,
//         data: TokenDataResponse { token: jwt_token },
//     }))
// }

// pub async fn info() -> Result<Json<Response<InfoResponse>>, StatusCode> {
//     let jwt_token = generate_jwt(&Claims {
//         username: "mining_proxy".to_string(),
//     })
//     .map_err(|err| {
//         log::error!("failed to generate jwt token: {}", err);
//         StatusCode::INTERNAL_SERVER_ERROR
//     })?;

//     Ok(Json(Response::<InfoResponse> {
//         code: 20000,
//         message: "".into(),
//         data: InfoResponse {
//             roles: vec!["admin".into()],
//             introduction: "".into(),
//             avatar: "".into(),
//             name: "admin".into(),
//         },
//     }))
// }

#[post("/user/login")]
async fn login(req: web::Json<LoginRequest>) -> actix_web::Result<impl Responder> {
    dbg!(req);

    Ok(web::Json(LoginResponse {
        code: 20000,
        data: TokenDataResponse {
            token: "123".to_string(),
        },
    }))
}

#[get("/user/info")]
async fn info() -> actix_web::Result<impl Responder> {
    Ok(web::Json(Response::<InfoResponse> {
        code: 20000,
        message: "".into(),
        data: InfoResponse {
            roles: vec!["admin".into()],
            introduction: "".into(),
            avatar: "".into(),
            name: "admin".into(),
        },
    }))
}

#[post("/crate/app")]
pub async fn crate_app(
    req: web::Json<CreateRequest>,
    app: web::Data<AppState>,
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

    // let exe_path = std::env::current_dir().expect("获取当前可执行程序路径错误");
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
            Err(e) => panic!(e),
        },
    };

    let mut configs = String::new();
    match cfgs.read_to_string(&mut configs) {
        Ok(_) => {
            let mut configs: Vec<Settings> = match serde_yaml::from_str(&configs) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("{}", e);
                    vec![]
                }
            };

            dbg!(configs.clone());
            configs.push(config.clone());

            dbg!(configs.clone());
            match serde_yaml::to_string(&configs) {
                Ok(mut c_str) => {
                    dbg!(c_str.clone());
                    c_str = c_str[4..c_str.len()].to_string();
                    drop(cfgs);
                    std::fs::remove_file("configs.yaml");
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
                            Err(e) => panic!(e),
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
                    app.global_count.lock().unwrap().insert(config.name, child);
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
                    app.global_count.lock().unwrap().insert(config.name, child);
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
async fn server_list(app: web::Data<AppState>) -> actix_web::Result<impl Responder> {
    let mut v = vec![];
    {
        let proxy_server = app.global_count.lock().unwrap();
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

// 展示选中的数据信息。以json格式返回
#[get("/user/server/{name}")]
async fn server(
    server: web::Path<String>,
    app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    log::debug!("{}", server);


    //let server = server.to_path().unwrap();

    let v = vec![];
    {
        let mut proxy_server = app.global_count.lock().unwrap();
        for (s, config) in &mut *proxy_server {
            log::info!("server {} ", s);
            if *s == server.to_string() {
                config.kill().await?;

            }
        }
    }

    //1. 基本配置文件信息 .
    //2. 抽水旷工信息     .
    //3. 当前在线矿机总数 .

    Ok(web::Json(Response::<Vec<String>> {
        code: 20000,
        message: "".into(),
        data: v,
    }))
}
