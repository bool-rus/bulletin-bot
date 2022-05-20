use std::sync::Arc;
use super::*;
use teloxide::types::BotCommand;

pub fn start(config: Arc<Config>) {
    let bot = Bot::new(config.token.as_str()).auto_send();
    let storage = MyStorage::new();
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
    .dependencies(dptree::deps![storage, config])
    .build();
    tokio::spawn(async move {
        let set_cmd = bot.set_my_commands([
            BotCommand::new("/help", "Помощь"), 
            BotCommand::new("/create", "Создать"), 
            BotCommand::new("/publish", "Опубликовать"), 
            ]).await;
        if let Err(e) = set_cmd {
            log::error!("Error on bot starting: {:?}", e);
            return
        }
        dispatcher.setup_ctrlc_handler()
        .dispatch()
        .await;
    });
}