use actix_web::{get, post, web, Responder};
use actix_web_grants::proc_macro::has_permissions;
use chrono::Utc;

use crate::web::{
    data::*,
    handles::auth::{generate_jwt, Claims},
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
    let iat = Utc::now();
    let exp = iat + chrono::Duration::days(1);
    if let Ok(jwt_token) = generate_jwt(Claims::new("mining_proxy".into(), exp))
    {
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
#[has_permissions("ROLE_ADMIN")]
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

#[post("/user/logout")]
#[has_permissions("ROLE_ADMIN")]
async fn logout() -> actix_web::Result<impl Responder> {
    Ok(web::Json(Response::<String> {
        code: 20000,
        message: "".into(),
        data: "".into(),
    }))
}
