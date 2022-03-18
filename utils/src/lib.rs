use crossterm::tty::IsTty;

pub fn initialize_logger(verbosity: u8) {
    match verbosity {
        0 => std::env::set_var("RUST_LOG", "info"),
        1 => std::env::set_var("RUST_LOG", "debug"),
        2 | 3 => std::env::set_var("RUST_LOG", "trace"),
        _ => std::env::set_var("RUST_LOG", "info"),
    };

    // struct LocalTimer;
    // impl FormatTime for LocalTimer {
    //     fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
    //         write!(w, "{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
    //     }
    // }

    // Filter out undesirable logs.
    // let filter = EnvFilter::from_default_env()
    //     .add_directive("mio=off".parse().unwrap())
    //     .add_directive("tokio_util=off".parse().unwrap())
    //     .add_directive("hyper::proto::h1::conn=off".parse().unwrap())
    //     .add_directive("hyper::proto::h1::decode=off".parse().unwrap())
    //     .add_directive("hyper::proto::h1::io=off".parse().unwrap())
    //     .add_directive("hyper::proto::h1::role=off".parse().unwrap())
    //     .add_directive("jsonrpsee=off".parse().unwrap());

    // Initialize tracing.
    let _ = tracing_subscriber::fmt()
        //        .with_env_filter(filter)
        .with_ansi(std::io::stdout().is_tty())
        //        .with_writer(move || LogWriter::new(&log_sender))
        .with_level(verbosity == 3)
        .with_target(verbosity == 3)
        .with_line_number(verbosity == 3)
        //.with_source_location(verbosity == 3)
        //.with_timer(LocalTimer)
        .try_init();
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
