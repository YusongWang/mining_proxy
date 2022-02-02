mod version {
    include!(concat!(env!("OUT_DIR"), "/version.rs"));
}

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    io::Read,
    sync::{atomic::AtomicUsize, Mutex},
};

use serde::{Deserialize, Serialize};
extern crate openssl_probe;

use actix_web::{get, post, web, App, HttpServer, Responder};
use mining_proxy::{
    client::{encry::accept_en_tcp, tcp::accept_tcp, tls::accept_tcp_with_tls},
    state::Worker,
    util::{config::Settings, logger},
    web::{AppState, OnlineWorker},
};

use anyhow::{bail, Result};
use bytes::{BufMut, BytesMut};
use clap::{crate_version, ArgMatches};
use human_panic::setup_panic;
use native_tls::Identity;

use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    select,
    sync::mpsc::{self, UnboundedReceiver},
};

use actix_web_static_files;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn main() -> Result<()> {
    setup_panic!();
    openssl_probe::init_ssl_cert_env_vars();
    let matches = mining_proxy::util::get_app_command_matches()?;
    if !matches.is_present("server") {
        actix_web::rt::System::with_tokio_rt(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                //.worker_threads(8)
                .thread_name("main-tokio")
                .build()
                .unwrap()
        })
        .block_on(async_main(matches))?;
    } else {
        //tokio::runtime::start
        tokio_main(&matches);
    }
    Ok(())
}

async fn async_main(matches: ArgMatches<'_>) -> Result<()> {
    logger::init_client(0)?;

    //let mut childs:HashMap<String,tokio::process::Child> =
    // HashMap::new();

    let mut data: AppState = std::sync::Arc::new(Mutex::new(HashMap::new()));

    match OpenOptions::new()
        .write(true)
        .read(true)
        //.create(true)
        //.truncate(true)
        .open("configs.yaml")
    {
        Ok(mut f) => {
            //let configs:Vec<Settings> = vec![];
            let mut configs = String::new();
            if let Ok(len) = f.read_to_string(&mut configs) {
                if len > 0 {
                    let configs: Vec<Settings> =
                        match serde_yaml::from_str(&configs) {
                            Ok(s) => s,
                            Err(e) => {
                                log::error!("{}", e);
                                vec![]
                            }
                        };
                    for config in configs {
                        match mining_proxy::util::run_server(&config) {
                            Ok(child) => {
                                let online = OnlineWorker {
                                    child,
                                    config: config.clone(),
                                    workers: vec![],
                                    online: 0,
                                };

                                data.lock()
                                    .unwrap()
                                    .insert(config.name, online);
                            }
                            Err(e) => {
                                log::error!("{}", e);
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {}
    };

    let tcp_data = data.clone();
    tokio::spawn(async move { recv_from_child(tcp_data).await });

    HttpServer::new(move || {
        /*  .service(
            web::scope("/api")
                .service(mining_proxy::web::handles::user::login)
                .service(mining_proxy::web::handles::user::crate_app)
                .service(mining_proxy::web::handles::user::server_list)
                .service(mining_proxy::web::handles::user::server)
                .service(mining_proxy::web::handles::user::info),
        ). */

        // .service(actix_web_static_files::ResourceFiles::new(
        //     "/", generated,
        // ))
        let generated = generate();
        let generated1 = generate();
        // for (g, res) in generated {
        //     dbg!(g);
        // }

        App::new()
            .app_data(web::Data::new(data.clone()))
            .service(
                web::scope("/api")
                    .service(mining_proxy::web::handles::user::login)
                    .service(mining_proxy::web::handles::user::crate_app)
                    .service(mining_proxy::web::handles::user::server_list)
                    .service(mining_proxy::web::handles::user::server)
                    .service(mining_proxy::web::handles::user::info),
            )
            .service(actix_web_static_files::ResourceFiles::new(
                "/", generated1,
            ))
            .service(actix_web_static_files::ResourceFiles::new("", generated))
    })
    .bind("0.0.0.0:8000")?
    .run()
    .await?;

    Ok(())
}

// fn actix_main(matches: ArgMatches<'static>) -> Result<()> {
//     // let mut rt = tokio::runtime::Runtime::new().unwrap();
//     // let local = tokio::task::LocalSet::new();
//     // local.block_on(&mut rt, async move {
//     //     tokio::task::spawn_local(async move {
//     //         let local = tokio::task::LocalSet::new();
//     //         let sys = actix_rt::System::run_in_tokio("server", &local);
//     //         // define your actix-web app
//     //         // define your actix-web server
//     //         sys.await;
//     //     });
//     //     // This still allows use of tokio::spawn
//     // });

//     //let mut data: AppState =
// std::sync::Arc::new(Mutex::new(HashMap::new()));     //let l =
// matches.clone();     //let a = data.clone();
//     actix_web::rt::System::new("a").block_on(async move {
//         actix_run(matches).await;
//     });

//     Ok(())
// }

async fn actix_run(matches: ArgMatches<'_>) -> Result<()> {
    logger::init_client(0)?;

    //let mut childs:HashMap<String,tokio::process::Child> =
    // HashMap::new();
    let mut data: AppState = std::sync::Arc::new(Mutex::new(HashMap::new()));

    match OpenOptions::new()
        .write(true)
        .read(true)
        //.create(true)
        //.truncate(true)
        .open("configs.yaml")
    {
        Ok(mut f) => {
            //let configs:Vec<Settings> = vec![];
            let mut configs = String::new();
            if let Ok(len) = f.read_to_string(&mut configs) {
                if len > 0 {
                    let configs: Vec<Settings> =
                        match serde_yaml::from_str(&configs) {
                            Ok(s) => s,
                            Err(e) => {
                                log::error!("{}", e);
                                vec![]
                            }
                        };
                    for config in configs {
                        match mining_proxy::util::run_server(&config) {
                            Ok(child) => {
                                let online = OnlineWorker {
                                    child,
                                    config: config.clone(),
                                    workers: vec![],
                                    online: 0,
                                };
                                data.lock()
                                    .unwrap()
                                    .insert(config.name, online);
                            }
                            Err(e) => {
                                log::error!("{}", e);
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {}
    };

    let tcp_data = data.clone();
    //let handle_tcp_connections = tokio::spawn(async move {
    // recv_from_child(tcp_data).await });
    let port:i32 = match std::env::var("MINING_PROXY_WEB_PORT") {
        Ok(p) => {
            p.parse().unwrap()
        },
        Err(_) => {
            8888
        }
    };


    let http = HttpServer::new(move || {
        /*  .service(
            web::scope("/api")
                .service(mining_proxy::web::handles::user::login)
                .service(mining_proxy::web::handles::user::crate_app)
                .service(mining_proxy::web::handles::user::server_list)
                .service(mining_proxy::web::handles::user::server)
                .service(mining_proxy::web::handles::user::info),
        ). */

        // .service(actix_web_static_files::ResourceFiles::new(
        //     "/", generated,
        // ))
        let generated = generate();

        App::new()
            .app_data(web::Data::new(data.clone()))
            .service(
                web::scope("/api")
                    .service(mining_proxy::web::handles::user::login)
                    .service(mining_proxy::web::handles::user::crate_app)
                    .service(mining_proxy::web::handles::user::server_list)
                    .service(mining_proxy::web::handles::user::server)
                    .service(mining_proxy::web::handles::user::info),
            )
            .service(actix_web_static_files::ResourceFiles::new("/", generated))
    })
    .bind(format!("0.0.0.0:{}",port))?
    .run()
    .await?;

    Ok(())
}

fn tokio_main(matches: &ArgMatches<'_>) {
    tokio::runtime::Builder::new_multi_thread()
        //.worker_threads(N)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { tokio_run(matches).await });
}

async fn tokio_run(matches: &ArgMatches<'_>) -> Result<()> {
    let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
    let config = Settings::new(config_file_name, true)?;

    logger::init(
        config.name.as_str(),
        config.log_path.clone(),
        config.log_level,
    )?;

    match config.check() {
        Ok(_) => {}
        Err(err) => {
            log::error!("config配置错误 {}", err);
            std::process::exit(1);
        }
    };

    let mut p12 = match File::open(config.p12_path.clone()).await {
        Ok(f) => f,
        Err(_) => {
            println!("证书路径错误: {} 下未找到证书!", config.p12_path);
            std::process::exit(1);
        }
    };

    let mode = if config.share == 0 {
        "纯代理模式"
    } else if config.share == 1 {
        "抽水模式"
    } else {
        "统一钱包模式"
    };

    log::info!("名称 {} 当前启动模式为: {}", config.name, mode);

    let mut buffer = BytesMut::with_capacity(10240);
    let read_key_len = p12.read_buf(&mut buffer).await?;
    let cert = Identity::from_pkcs12(
        &buffer[0..read_key_len],
        config.p12_pass.clone().as_str(),
    )?;

    let (worker_tx, worker_rx) = mpsc::unbounded_channel::<Worker>();

    let state =
        std::sync::Arc::new(mining_proxy::state::GlobalState::default());

    let res = tokio::try_join!(
        accept_tcp(worker_tx.clone(), config.clone(), state.clone()),
        accept_en_tcp(worker_tx.clone(), config.clone(), state.clone()),
        accept_tcp_with_tls(
            worker_tx.clone(),
            config.clone(),
            cert,
            state.clone()
        ),
        send_to_parent(worker_rx, &config),
    );

    if let Err(err) = res {
        log::error!("致命错误 : {}", err);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendToParentStruct {
    name: String,
    worker: Worker,
}

async fn send_to_parent(
    mut worker_rx: UnboundedReceiver<Worker>, config: &Settings,
) -> Result<()> {
    let runtime = std::time::Instant::now();

    loop {
        if let Ok(mut stream) =
            tokio::net::TcpStream::connect("127.0.0.1:65500").await
        {
            let sleep =
                tokio::time::sleep(tokio::time::Duration::from_secs(5 * 60));
            tokio::pin!(sleep);
            //RPC impl
            // let a = "hello ".to_string();
            // let mut b = a.as_bytes().to_vec();
            // b.push(b'\n');
            // stream.write(&b).await.unwrap();
            let name = config.name.clone();

            select! {
                Some(w) = worker_rx.recv() => {
                    let send = SendToParentStruct{
                        name:name,
                        worker:w,
                    };

                    let mut rpc = serde_json::to_vec(&send)?;
                    rpc.push(b'\n');
                    stream.write(&rpc).await.unwrap();
                },
                () = &mut sleep => {
                    //RPC keep.alive
                    //一分钟发送一次保持活动
                    sleep.as_mut().reset(tokio::time::Instant::now() + tokio::time::Duration::from_secs(60));
                },
            }
        } else {
            log::error!("无法链接到主控web端");
            tokio::time::sleep(tokio::time::Duration::from_secs(60 * 2)).await;
        }
    }
}

async fn recv_from_child(app: AppState) -> Result<()> {
    let address = "127.0.0.1:65500";
    let listener = match tokio::net::TcpListener::bind(address.clone()).await {
        Ok(listener) => listener,
        Err(_) => {
            log::info!("本地端口被占用 {}", address);
            std::process::exit(1);
        }
    };

    log::info!("本地TCP端口{} 启动成功!!!", &address);
    println!("监听本机端口{}", 65500);
    loop {
        let (mut stream, _) = listener.accept().await?;
        let inner_app = app.clone();
        //r_lines.next_line() => {}
        //let mut pool_lines = pool_r.lines();
        tokio::spawn(async move {
            let (mut r, _) = stream.split();
            let mut r_buf = BufReader::new(r);
            let mut r_lines = r_buf.lines();

            loop {
                let mut buf: BytesMut = BytesMut::new();

                if let Ok(Some(buf_str)) = r_lines.next_line().await {
                    let s = String::from_utf8(buf.to_vec()).unwrap();
                    log::info!("{}", s);
                    log::info!("-----------------------");
                    if let Ok(online_work) =
                        serde_json::from_str::<SendToParentStruct>(&buf_str)
                    {
                        dbg!(&online_work);

                        if let Some(temp_app) =
                            inner_app.lock().unwrap().get_mut(&online_work.name)
                        {
                            let mut is_update = false;
                            for worker in &mut temp_app.workers {
                                if worker.worker == online_work.worker.worker {
                                    dbg!(&worker);
                                    *worker = online_work.worker.clone();
                                    is_update = true;
                                }
                            }
                            if is_update == false {
                                temp_app.workers.push(online_work.worker);
                            }
                        } else {
                            log::error!("未找到此端口");
                        }
                    }
                };
            }
        });
    }

    Ok(())
}
