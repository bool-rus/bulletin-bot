use teloxide::update_listeners::UpdateListener;

use super::*;

mod fsm;
mod entity;

type MyStorage = teloxide::dispatching::dialogue::InMemStorage<fsm::State>;

pub fn start(sender: Sender<DBAction>, db: DBStorage, started_bots: StartedBots) -> tokio::task::JoinHandle<()> {
    let bot = Bot::new(CONF.token.as_str());
    tokio::spawn(async move {
        bot.set_my_commands(fsm::bot_commands()).await.ok_or_log();
        bot.send_message(ChatId(CONF.admin as i64), "Запускаюсь...").await.ok_or_log();

        let mut listener = teloxide::dispatching::update_listeners::polling_default(bot.clone()).await;
        let stop_token = listener.stop_token();
        let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
            .dependencies(dptree::deps![MyStorage::new(), Arc::new(sender), db, started_bots.clone(), stop_token])
            .enable_ctrlc_handler()
            .build();
        dispatcher.dispatch_with_listener(listener, LoggingErrorHandler::with_custom_text("father bot err:")).await;
        
        let tokens: Vec<_> = started_bots.lock().unwrap().values().cloned().collect();
        let shutdowns = tokens.iter().filter_map(|token|token.shutdown().ok());
        futures_util::future::join_all(shutdowns).await;
    })
}
