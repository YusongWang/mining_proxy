use anyhow::Result;
use log::{info};

mod util;
extern crate clap;
use clap::{
     crate_authors, crate_description, crate_name, crate_version, App, Arg, ArgMatches,
};

async fn get_app_command_matches() -> Result<ArgMatches<'static>> {
     let matches = App::new(crate_name!())
          .version(crate_version!())
          .author(crate_authors!("\n"))
          .about(crate_description!())
          .arg(Arg::with_name("config")
               .short("c")
               .long("config")
               .value_name("FILE")
               .help("Sets a custom config file")
               .takes_value(true))
          .get_matches();
     Ok(matches)
}

#[tokio::main]
async fn main() ->Result<()>{
     let matches = get_app_command_matches().await?;
     let config_file_name = matches.value_of("config").unwrap_or("default.yaml");
     let config = util::config::Settings::new(config_file_name)?;
     util::logger::init("proxy",&config.log_path,&config.log_level)?;
     info!("config init success!");

     Ok(())
}
