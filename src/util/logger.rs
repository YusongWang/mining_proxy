pub fn init(app_name: &str, path: String, log_level: u32) -> anyhow::Result<()> {
    let lavel = match log_level {
        4 => log::LevelFilter::Off,
        3 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        0 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    };

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
        .level_for("reqwest", log::LevelFilter::Off)
        .chain(std::io::stdout())
        .chain(log)
        .into_log();

    let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
        log::Level::Error => sentry_log::LogFilter::Event,
        log::Level::Warn => sentry_log::LogFilter::Event,
        _ => sentry_log::LogFilter::Ignore,
    });

    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(lavel);

    Ok(())
}
