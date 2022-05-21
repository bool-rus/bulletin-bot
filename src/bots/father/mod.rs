
use std::sync::Arc;

use super::*;
use bulletin::Config as BulletinConfig;
mod fsm;

use crate::impls::LoggableErrorResult;
type MyStorage = teloxide::dispatching::dialogue::InMemStorage<fsm::State>;

pub fn start(token: String, sender: Sender<DBAction>) {
    let bot = Bot::new(token.as_str()).auto_send();
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
    .dependencies(dptree::deps![MyStorage::new(), Arc::new(sender)])
    .build();
    tokio::spawn(async move {
        bot.set_my_commands(fsm::bot_commands()).await.ok_or_log();
        dispatcher.setup_ctrlc_handler()
        .dispatch()
        .await;
    });
}