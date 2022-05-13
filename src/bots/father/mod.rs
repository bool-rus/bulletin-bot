
use super::*;
use bulletin::Config as BulletinConfig;
mod fsm;

type MyStorage = teloxide::dispatching::dialogue::InMemStorage<fsm::State>;

pub fn start(token: String) {
    let bot = Bot::new(token.as_str()).auto_send();
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
    .dependencies(dptree::deps![MyStorage::new()])
    .build();
    tokio::spawn(async move {
        dispatcher.setup_ctrlc_handler()
        .dispatch()
        .await;
    });
}