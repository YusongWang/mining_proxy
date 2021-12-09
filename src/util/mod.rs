pub mod config;

pub mod logger {

    pub fn init(
        app_name: &'static str,
        path: String,
        log_level: i32,
    ) -> Result<(), fern::InitError> {
        // parse log_laver
        let lavel = match log_level {
            3 => log::LevelFilter::Error,
            2 => log::LevelFilter::Info,
            1 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Debug,
        };

        let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
            .utc_time()
            .local_time();
        fern::Dispatch::new()
            .format(move |out, message, record| {
                out.finish(format_args!(
                    "[{}] [{}] [{}:{}] [{}] {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    app_name,
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
            .apply()?;
        Ok(())
    }
}
