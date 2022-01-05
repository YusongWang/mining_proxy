use actix_web::{web, App, HttpRequest, HttpServer, Responder};
use proxy::{util, web::webmain::{init_worker_rt}};


async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");

    format!("Hello {}!", &name)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    util::logger::init("web", "".to_string(), 0).unwrap();
    //let worker_queue =
    let (s, r) = async_channel::unbounded::<String>();
    s.send("default.yaml".into()).await.unwrap();

    let http_server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(greet))
            .route("/{name}", web::get().to(greet))
    })
    .bind(("127.0.0.1", 8081))?
    .run();

    let proxy_worker = init_worker_rt(r);

    let res = tokio::try_join!(http_server, proxy_worker);
    // match a {
    //     Ok(_) => {return Ok(())},
    //     Err(e) => {
    //         return std::io::Error::new(std::io::ErrorKind::Other);
    //     },
    // }
    if let Err(err) = res {
        log::error!("致命错误 : {}", err);
        //return std::io::Error::new(std::io::ErrorKind::Other,err);
    }

    return Ok(());
}
