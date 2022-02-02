use std::{
    fs::OpenOptions,
    io::{Read, Write},
};

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};

use crate::{
    state::Worker,
    util::config::Settings,
    web::{
        data::*,
        handles::auth::{generate_jwt, Claims},
        AppState, OnlineWorker,
    },
};

#[post("/user/login")]
async fn login(
    req: web::Json<LoginRequest>,
) -> actix_web::Result<impl Responder> {
    let password = match std::env::var("MINING_PROXY_WEB_PASSWORD") {
        Ok(t) => t,
        Err(_) => "admin123".into(),
    };

    if password != req.password {
        return Ok(web::Json(Response::<TokenDataResponse> {
            code: 40000,
            message: "密码不正确".into(),
            data: TokenDataResponse::default(),
        }));
    }

    if let Ok(jwt_token) = generate_jwt(&Claims {
        username: "mining_proxy".to_string(),
    }) {
        Ok(web::Json(Response::<TokenDataResponse> {
            code: 20000,
            message: "".into(),
            data: TokenDataResponse { token: jwt_token },
        }))
    } else {
        Ok(web::Json(Response::<TokenDataResponse> {
            code: 40000,
            message: "生成token失败".into(),
            data: TokenDataResponse::default(),
        }))
    }
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
