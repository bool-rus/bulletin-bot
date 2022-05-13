mod impls;
mod bots;

#[tokio::main]
async fn main() {
    init_logger();
    bots::father::start("1664451950:AAFKLe7bVhzbjJ-G1aoDubjbNBCRQffntE0".into());
    tokio::signal::ctrl_c().await.expect("Failed to listen for ^C");
    //sleep(std::time::Duration::from_secs(5)).await;
}

fn init_logger() {
    use simplelog::*;
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
}
