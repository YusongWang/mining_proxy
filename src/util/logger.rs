pub fn init(app_name: &str, path: String, log_level: u32) -> anyhow::Result<()> {
    // parse log_laver
    let lavel = match log_level {
        3 => log::LevelFilter::Error,
        2 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    };

    //if log_level <= 1 {
    let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
        .utc_time()
        .local_time();
    let (lavel, logger) = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}] [{}:{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.file().unwrap(),
                record.line().unwrap(),
                record.level(),
                message
            ))
        })
        .level(lavel)
        //.level_for("engine", log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(log)
        .into_log();

    let logger = sentry_log::SentryLogger::with_dest(logger);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(lavel);
    // } else {
    //     let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
    //         .utc_time()
    //         .local_time();
    //     fern::Dispatch::new()
    //         .format(move |out, message, record| {
    //             out.finish(format_args!(
    //                 "[{}] [{}] {}",
    //                 chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
    //                 record.level(),
    //                 message
    //             ))
    //         })
    //         .level(lavel)
    //         //.level_for("engine", log::LevelFilter::Debug)
    //         .chain(std::io::stdout())
    //         .chain(log)
    //         .into_log();
    // }
    //use sentry_tracing;
    //use tracing_subscriber::prelude::*;

    //tracing_subscriber::registry().init();
    //.unwrap();
    // log::set_boxed_logger(Box::new(logger)).unwrap();
    // log::set_max_level(log::LevelFilter::Info);

    let _guard = sentry::init((
        "https://a9ae2ec4a77c4c03bca2a0c792d5382b@o1095800.ingest.sentry.io/6115709",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    Ok(())
}
