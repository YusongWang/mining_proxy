use serde::{Deserialize, Serialize};

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
    pub username: String,
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

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};

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

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
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

// pub async fn crate_app(
//     extract::Json(req): extract::Json<CreateRequest>,
// ) -> Result<Json<Response<InfoResponse>>, StatusCode> {
//     dbg!(req.clone());
//     let mut config = Settings::default();
//     if req.name == "" {
//         return Ok(Json(Response::<InfoResponse> {
//             code: 40000,
//             message:"中转名称必须填写".into(),
//             data: InfoResponse::default()
//         }))
//     }

//     if req.tcp_port == 0  && req.ssl_port == 0  && req.encrypt_port == 0{
//         return Ok(Json(Response::<InfoResponse> {
//             code: 40000,
//             message:"未开启端口。请至少开启一个端口".into(),
//             data: InfoResponse::default()
//         }))
//     }

//     if req.pool_address.is_empty() {
//         //println!("中转矿池必须填写");
//         return Ok(Json(Response::<InfoResponse> {
//             code: 40000,
//             message:"中转矿池必须填写".into(),
//             data: InfoResponse::default()
//         }))
//     }

//     if req.share != 0 {
//         if req.share_address.is_empty() {
//             //println!("抽水矿池必须填写");
//             return Ok(Json(Response::<InfoResponse> {
//                 code: 40000,
//                 message:"抽水矿池必须填写".into(),
//                 data: InfoResponse::default()
//             }))
//         }

//         if req.share_wallet.is_empty() {
//             //println!("抽水钱包必须填写");
//             return Ok(Json(Response::<InfoResponse> {
//                 code: 40000,
//                 message:"抽水钱包必须填写".into(),
//                 data: InfoResponse::default()
//             }))
//         }

//         if req.share_rate == 0 {
//             //println!("抽水比例必须填写");
//             return Ok(Json(Response::<InfoResponse> {
//                 code: 40000,
//                 message:"抽水比例必须填写".into(),
//                 data: InfoResponse::default()
//             }))
//         }
//     }

//     config.share_name = req.name.clone();
//     config.name = req.name;
//     config.pool_address = vec![req.pool_address];
//     config.share_address = vec![req.share_address];
//     config.tcp_port = req.tcp_port;
//     config.ssl_port = req.ssl_port;
//     config.encrypt_port = req.encrypt_port;
//     config.share = req.share;
//     config.share_rate = req.share_rate as f32 / 100.0;
//     config.share_wallet = req.share_wallet;
//     config.key = req.key;
//     config.iv = req.iv;

//     match config.check() {
//         Ok(_) => {},
//         Err(err) => {
//             log::error!("config配置错误 {}",err);
//             return Ok(Json(Response::<InfoResponse> {
//                 code: 40000,
//                 message: format!("config配置错误 {}",err),
//                 data: InfoResponse::default()
//             }))
//             //std::process::exit(1);
//         },
//     };

//     //tokio::process::Command::new(program)
//     let exe = std::env::current_exe().expect("无法获取当前可执行程序路径");
//     let exe_path = std::env::current_dir().expect("获取当前可执行程序路径错误");

//     let mut handle = tokio::process::Command::new(exe);

//     let mut handle = handle.arg("--server")
//     .env("PROXY_NAME", config.name)
//     .env("PROXY_LOG_LEVEL", "1".to_string())
//     .env("PROXY_LOG_PATH", "./logs/")
//     .env("PROXY_TCP_PORT", config.tcp_port.to_string())
//     .env("PROXY_SSL_PORT", config.ssl_port.to_string())
//     .env("PROXY_ENCRYPT_PORT", config.encrypt_port.to_string())
//     .env("PROXY_TCP_ADDRESS", config.pool_address[0].clone())
//     .env("PROXY_SHARE_ADDRESS", config.share_address[0].clone())
//     .env("PROXY_SHARE_RATE", config.share_rate.to_string())
//     .env("PROXY_SHARE_NAME", config.share_name.to_string())
//     .env("PROXY_SHARE", config.share.to_string())
//     .env("PROXY_P12_PATH", exe_path.to_str().expect("无法转换路径为字符串").to_string()+"/identity.p12")
//     .env("PROXY_P12_PASS", "mypass".to_string())
//     .env("PROXY_KEY", config.key.to_string())
//     .env("PROXY_IV", config.iv.to_string());

//     let Child = match handle.spawn() {
//         Ok(t) => t,
//         Err(e) => {
//             return Ok(Json(Response::<InfoResponse> {
//                 code: 40000,
//                 message: "".into(),
//                 data: InfoResponse::default()
//             }))
//         }
//     };

//     // let code = Child
//     //     .wait()
//     //     .await
//     //     .expect("无法获得状态码")
//     //     .code()
//     //     .expect("无法获取返回码");
//     // if code != 0 {
//     //     println!("{}", code);
//     // }

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
