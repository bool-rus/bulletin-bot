use clap::{Parser, command};


mod impls;
mod bots;
mod persistent;

#[ctor::ctor]
#[cfg(not(test))]
pub static CONF: GlobalConfig = GlobalConfig::parse();

#[tokio::main]
async fn main() {
    init_logger();
    bots::start().await;
}

fn init_logger() {
    use simplelog::*;
    let mut builder = ConfigBuilder::new();
    builder.set_time_level(LevelFilter::Off);
    TermLogger::init(LevelFilter::Info, builder.build(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
    log::info!("config: {:?}", &CONF.admin);
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct GlobalConfig {
    ///token of father bot 
    #[arg(long, short, env="TELEGRAM_BOT_TOKEN", hide_env_values=true)]
    pub token: String,
    ///id of global admin
    #[arg(long, short, env="BOT_ADMIN")]
    admin: u64,
    ///tip url
    #[arg(long, env="TIP_URL")]
    pub tip_url: String,
    ///path to db file
    #[arg(long="db", default_value="bulletin-configs.db")]
    db_path: String,
}

#[ctor::ctor]
#[cfg(test)]
pub static CONF: GlobalConfig = GlobalConfig {
    token: "test".to_string(),
    admin: 0,
    tip_url: "https://example.com".to_string(),
    db_path: "test.db".to_string(),
};