use std::sync::Arc;

use crate::impls::LoggableErrorResult;

use super::*;
use teloxide::types::{UserId, ChatId, BotCommand, ParseMode};

pub struct Config {
    pub token: String, 
    pub admin_ids: Vec<UserId>,
    pub channel: ChatId,
}

impl Config {
    pub fn is_admin(&self, user_id: &UserId) -> bool {
        self.admin_ids.contains(user_id)
    }
}

pub fn start(config: Config) {
    let config = Arc::new(config);
    let bot = Bot::new(config.token.as_str()).auto_send();
    let storage = Storage::new();
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
    .dependencies(dptree::deps![storage, config])
    .build();
    tokio::spawn(async move {
        bot.set_my_commands([
            BotCommand::new("/help", "Помощь"), 
            BotCommand::new("/create", "Создать"), 
            BotCommand::new("/publish", "Опубликовать"), 
            ]).await.ok_or_log();
        dispatcher.setup_ctrlc_handler()
        .dispatch()
        .await;
    });
}