pub fn init(app_name: &str, path: String, log_level: u32) -> anyhow::Result<()> {
    let lavel = match log_level {
        4 => log::LevelFilter::Off,
        3 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        0 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    };
    cfg_if::cfg_if! {
        if #[cfg(debug_assertions)] {
            if path != "" {
                let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
                    .utc_time()
                    .local_time();
                let (lavel, logger) = fern::Dispatch::new()
                    .format(move |out, message, record| {
                        out.finish(format_args!(
                            "[{}] [{}] [{}:{}] {}",
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                            record.level(),
                            record.file().expect("获取文件名称失败"),
                            record.line().expect("获取文件行号失败"),
                            message
                        ))
                    })
                    .level(lavel)
                    .level_for("reqwest", log::LevelFilter::Off)
                    .chain(std::io::stdout())
                    .chain(log)
                    .into_log();

                // let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
                //     log::Level::Error => sentry_log::LogFilter::Event,
                //     log::Level::Warn => sentry_log::LogFilter::Event,
                //     _ => sentry_log::LogFilter::Ignore,
                // });

                log::set_boxed_logger(Box::new(logger)).unwrap();
                log::set_max_level(lavel);
            } else {
                let (lavel, logger) = fern::Dispatch::new()
                    .format(move |out, message, record| {
                        out.finish(format_args!(
                            "[{}] [{}] [{}:{}] {}",
                            record.level(),
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                            record.file().expect("获取文件名称失败"),
                            record.line().expect("获取文件行号失败"),
                            message
                        ))
                    })
                    .level(lavel)
                    .level_for("reqwest", log::LevelFilter::Off)
                    .chain(std::io::stdout())
                    .into_log();

                // let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
                //     log::Level::Error => sentry_log::LogFilter::Event,
                //     log::Level::Warn => sentry_log::LogFilter::Event,
                //     _ => sentry_log::LogFilter::Ignore,
                // });

                log::set_boxed_logger(Box::new(logger)).unwrap();
                log::set_max_level(lavel);
            }
        }  else {

            if path != "" {
                let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
                    .utc_time()
                    .local_time();
                let (lavel, logger) = fern::Dispatch::new()
                    .format(move |out, message, record| {
                        out.finish(format_args!(
                            "[{}] [{}] {}",
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                            record.level(),
                            message
                        ))
                    })
                    .level(lavel)
                    .level_for("reqwest", log::LevelFilter::Off)
                    .chain(std::io::stdout())
                    .chain(log)
                    .into_log();

                // let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
                //     log::Level::Error => sentry_log::LogFilter::Event,
                //     log::Level::Warn => sentry_log::LogFilter::Event,
                //     _ => sentry_log::LogFilter::Ignore,
                // });

                log::set_boxed_logger(Box::new(logger)).unwrap();
                log::set_max_level(lavel);
            } else {
                let (lavel, logger) = fern::Dispatch::new()
                    .format(move |out, message, record| {
                        out.finish(format_args!(
                            "[{}] [{}] {}",
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                            record.level(),
                            message
                        ))
                    })
                    .level(lavel)
                    .level_for("reqwest", log::LevelFilter::Off)
                    .chain(std::io::stdout())
                    .into_log();

                // let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
                //     log::Level::Error => sentry_log::LogFilter::Event,
                //     log::Level::Warn => sentry_log::LogFilter::Event,
                //     _ => sentry_log::LogFilter::Ignore,
                // });

                log::set_boxed_logger(Box::new(logger)).unwrap();
                log::set_max_level(lavel);
            }
        }
    }

    Ok(())
}

pub fn init_client(log_level: u32) -> anyhow::Result<()> {
    let lavel = match log_level {
        4 => log::LevelFilter::Off,
        3 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        0 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    };

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
        .into_log();

    // let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
    //     log::Level::Error => sentry_log::LogFilter::Event,
    //     log::Level::Warn => sentry_log::LogFilter::Event,
    //     _ => sentry_log::LogFilter::Ignore,
    // });

    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(lavel);

    Ok(())
}
