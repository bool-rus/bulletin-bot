use super::*;

mod fsm;
mod entity;

type MyStorage = teloxide::dispatching::dialogue::InMemStorage<fsm::State>;

pub fn start(sender: Sender<DBAction>, db: DBStorage, started_bots: StartedBots) -> tokio::task::JoinHandle<()> {
    let bot = Bot::new(CONF.token.as_str());
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
    .dependencies(dptree::deps![MyStorage::new(), Arc::new(sender), db, started_bots.clone()])
    .enable_ctrlc_handler()
    .build();
    tokio::spawn(async move {
        bot.set_my_commands(fsm::bot_commands()).await.ok_or_log();
        dispatcher.dispatch().await;
        let tokens: Vec<_> = started_bots.lock().unwrap().values().cloned().collect();
        let shutdowns = tokens.iter().filter_map(|token|token.shutdown().ok());
        futures_util::future::join_all(shutdowns).await;
    })
}
