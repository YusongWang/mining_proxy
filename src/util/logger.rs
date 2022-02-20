// pub fn init(
//     app_name: &str, path: String, log_level: u32,
// ) -> anyhow::Result<()> {
//     let lavel = match log_level {
//         4 => log::LevelFilter::Off,
//         3 => log::LevelFilter::Error,
//         2 => log::LevelFilter::Warn,
//         1 => log::LevelFilter::Info,
//         0 => log::LevelFilter::Debug,
//         _ => log::LevelFilter::Info,
//     };
//     cfg_if::cfg_if! {
//         if #[cfg(debug_assertions)] {
//             if path != "" {
//                 let log = fern::DateBased::new(path,
// format!("{}.log.%Y-%m-%d.%H", app_name))                     .utc_time()
//                     .local_time();
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] [{}:{}] {}",
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.level(),
//                             record.file().expect("获取文件名称失败"),
//                             record.line().expect("获取文件行号失败"),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .chain(log)
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             } else {
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] [{}:{}] {}",
//                             record.level(),
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.file().expect("获取文件名称失败"),
//                             record.line().expect("获取文件行号失败"),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             }
//         }  else {

//             if path != "" {
//                 let log = fern::DateBased::new(path,
// format!("{}.log.%Y-%m-%d.%H", app_name))                     .utc_time()
//                     .local_time();
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] {}",
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.level(),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .chain(log)
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             } else {
//                 let (lavel, logger) = fern::Dispatch::new()
//                     .format(move |out, message, record| {
//                         out.finish(format_args!(
//                             "[{}] [{}] {}",
//                             chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                             record.level(),
//                             message
//                         ))
//                     })
//                     .level(lavel)
//                     .level_for("reqwest", log::LevelFilter::Off)
//                     .chain(std::io::stdout())
//                     .into_log();

//                 // let logger =
// sentry_log::SentryLogger::with_dest(logger).filter(|md| match md.level() {
//                 //     log::Level::Error => sentry_log::LogFilter::Event,
//                 //     log::Level::Warn => sentry_log::LogFilter::Event,
//                 //     _ => sentry_log::LogFilter::Ignore,
//                 // });

//                 log::set_boxed_logger(Box::new(logger)).unwrap();
//                 log::set_max_level(lavel);
//             }
//         }
//     }

//     Ok(())
// }

// pub fn init_client(log_level: u32) -> anyhow::Result<()> {
//     let lavel = match log_level {
//         4 => log::LevelFilter::Off,
//         3 => log::LevelFilter::Error,
//         2 => log::LevelFilter::Warn,
//         1 => log::LevelFilter::Info,
//         0 => log::LevelFilter::Debug,
//         _ => log::LevelFilter::Info,
//     };

//     let (lavel, logger) = fern::Dispatch::new()
//         .format(move |out, message, record| {
//             out.finish(format_args!(
//                 "[{}] [{}:{}] [{}] {}",
//                 chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
//                 record.file().unwrap(),
//                 record.line().unwrap(),
//                 record.level(),
//                 message
//             ))
//         })
//         .level(lavel)
//         .level_for("reqwest", log::LevelFilter::Off)
//         .chain(std::io::stdout())
//         .into_log();

//     // let logger = sentry_log::SentryLogger::with_dest(logger).filter(|md|
//     // match md.level() {     log::Level::Error =>
//     // sentry_log::LogFilter::Event,     log::Level::Warn =>
//     // sentry_log::LogFilter::Event,     _ => sentry_log::LogFilter::Ignore,
//     // });

//     log::set_boxed_logger(Box::new(logger)).unwrap();
//     log::set_max_level(lavel);

//     Ok(())
// }

use std::io;

use tracing::Level;
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

pub fn init() {
    struct LocalTimer;

    impl FormatTime for LocalTimer {
        fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
            write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
        }
    }

    // let file_appender =
    //     tracing_appender::rolling::daily("./logs/", "AProxy.log");
    // let (non_blocking, _guard) =
    // tracing_appender::non_blocking(file_appender);

    // 设置日志输出时的格式，例如，是否包含日志级别、是否包含日志来源位置、
    // 设置日志的时间格式 参考: https://docs.rs/tracing-subscriber/0.3.3/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.with_timer
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(false)
        .with_line_number(true)
        .with_source_location(true)
        .with_timer(LocalTimer);

    // 初始化并设置日志格式(定制和筛选日志)
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_writer(io::stdout) // 写入标准输出
        //.with_writer(non_blocking) // 写入文件，将覆盖上面的标准输出
        //.with_ansi(false) // 如果日志是写入文件，应将ansi的颜色输出功能关掉
        .event_format(format)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");
}
