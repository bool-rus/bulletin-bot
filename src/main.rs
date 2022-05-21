mod impls;
mod bots;
mod persistent;

#[tokio::main]
async fn main() {
    init_logger();
    let (sender, configs) = persistent::worker().await;
    configs.into_iter().for_each(|conf|bots::bulletin::start(conf.into()));
    bots::father::start(
        std::env::var("TELEGRAM_BOT_TOKEN").expect("need to set env variable TELEGRAM_BOT_TOKEN"), 
        sender
    );
    tokio::signal::ctrl_c().await.expect("Failed to listen for ^C");
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
}

fn init_logger() {
    use simplelog::*;
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
}
