use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use impls::LoggableErrorResult;
use persistent::DBAction::AddListener;
use teloxide::dispatching::ShutdownToken;

mod impls;
mod bots;
mod persistent;
type StartedBots = Arc<Mutex<HashMap<i64, ShutdownToken>>>;

#[tokio::main]
async fn main() {
    init_logger();
    let (sender, configs, storage) = persistent::worker().await;
    let started_bots = configs.into_iter().fold(HashMap::new(),|mut map, (id, conf)|{
        let conf: bots::bulletin::Config = conf.into();
        let receiver = conf.receiver.clone();
        map.insert(id, bots::bulletin::start(conf));
        sender.send(AddListener(id, receiver)).unwrap();
        map
    });
    bots::father::start(
        std::env::var("TELEGRAM_BOT_TOKEN").expect("need to set env variable TELEGRAM_BOT_TOKEN"), 
        sender,
        storage.clone(),
        Arc::new(Mutex::new(started_bots))
    ).await.ok_or_log();
    storage.close().await;
}

fn init_logger() {
    use simplelog::*;
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
}
