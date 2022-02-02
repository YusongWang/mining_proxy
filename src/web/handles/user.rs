use std::{
    fs::OpenOptions,
    io::{Read, Write},
};

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};

use crate::{
    state::Worker,
    util::config::Settings,
    web::{data::*, AppState, OnlineWorker},
};

#[post("/user/login")]
async fn login(
    req: web::Json<LoginRequest>,
) -> actix_web::Result<impl Responder> {
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
