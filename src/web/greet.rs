use std::{env, path::PathBuf};

use crate::web::{actions, run};
use actix_files::NamedFile;
use actix_web::{web, HttpRequest, HttpResponse, Responder, Result};
use log::info;

use super::DbPool;

pub async fn index(req: HttpRequest) -> Result<NamedFile> {
    Ok(NamedFile::open("./static/index.html")?)
}

pub async fn greet(req: HttpRequest, pool: web::Data<DbPool>) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    // let a = db.state();
    // info!("{:?}",a);
    let pool1 = pool.clone();
    let user = web::block(move || {
        let conn = pool.get()?;
        actions::find_user_by_uid(&conn)
    })
    .await
    .map_err(|e| {
        eprintln!("{}", e);
        HttpResponse::InternalServerError().finish()
    })
    .unwrap()
    .unwrap();
    let db = pool1.get().unwrap();
    run(user, db);

    format!("Hello {}!", &name)
}
