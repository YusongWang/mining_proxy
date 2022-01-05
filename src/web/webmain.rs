use std::time::Duration;

use actix_files as fs;
use actix_web::{web, App, HttpServer};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use futures::future;
use log::info;

use tokio::select;

use super::{
    greet::{greet, index, admin_info, pool_list},
    DbPool,
};

pub async fn init_worker_rt(r: Receiver<String>) -> std::io::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        //.worker_threads(8) // 8个工作线程
        .enable_io() // 可在runtime中使用异步IO
        .enable_time() // 可在runtime中使用异步计时器(timer)
        .build() // 创建runtime
        .unwrap();

    rt.block_on(worker(r));

    Ok(())
}

pub async fn init_worker(r: Receiver<String>) -> std::io::Result<()> {
    match worker(r).await {
        Ok(_) => Ok(()),
        Err(e) => {
            info!("init_worker {}", e);
            Ok(())
        }
    }
}




pub async fn init_http(pool: DbPool) -> std::io::Result<()> {
    let sys = actix_rt::System::new("web");
    let http_server = HttpServer::new(move || {
        App::new()
            //.data(AppState { queue: s.clone() })
            .data(pool.clone())
            .service(admin_info)
            .service(pool_list)
            .route("/", web::get().to(index))
            .service(fs::Files::new("/", "./static/").show_files_listing())
        
        // .route("/", web::get().to(greet))
        // .route("/{name}", web::get().to(greet))
    })
    .bind(("127.0.0.1", 8081))?
    .run();

    sys.run()
}

pub async fn worker(r: Receiver<String>) -> Result<()> {
    let mut v = vec![];

    for _ in 0..5 {
        let r = r.clone();
        v.push(async move {
            loop {
                select! {
                    recv = r.recv() => {
                        let config_name = match recv {
                            Ok(s) => s,
                            Err(e) => {
                                panic!("failed to recv {}",e);
                            },
                        };

                        info!("worker获得工作 {}",config_name);

                        let mut p = std::process::Command::new("target/debug/proxy")
                        .arg("-c")
                        .arg(config_name).spawn().unwrap();

                        //let mut a = p.spawn().unwrap();
                        //p.wait().unwrap();
                        tokio::time::sleep(Duration::new(2, 0)).await;
                    }
                }
            }
        });
    }

    let res = future::try_join_all(v.into_iter().map(tokio::spawn)).await;
    if let Err(e) = res {
        log::warn!("服务 :{}", e);
    }

    Ok(())
}
