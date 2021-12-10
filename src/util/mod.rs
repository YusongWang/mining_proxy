pub mod config;

extern crate clap;
use anyhow::Result;
use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches};

pub async fn get_app_command_matches() -> Result<ArgMatches<'static>> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(crate_description!())
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .get_matches();
    Ok(matches)
}

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
            _ => log::LevelFilter::Info,
        };

        let log = fern::DateBased::new(path, format!("{}.log.%Y-%m-%d.%H", app_name))
            .utc_time()
            .local_time();
        fern::Dispatch::new()
            .format(move |out, message, record| {
                out.finish(format_args!(
                    "[{}] [{}:{}] [{}] {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    //record.target(),
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
