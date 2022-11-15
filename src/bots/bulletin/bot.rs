use std::sync::Arc;
use crate::impls::LoggableErrorResult;

use super::*;
use teloxide::{types::BotCommand, dispatching::ShutdownToken};

pub fn start(config: Config) -> ShutdownToken {
    let config = Arc::new(config);
    let bot = Bot::new(config.token.as_str());
    let storage = MyStorage::new();
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
        .dependencies(dptree::deps![storage, config.clone()])
        .build();
    let token = dispatcher.shutdown_token();
    tokio::spawn(async move {

        let me = bot.get_me().await.ok_or_log();
        let channel = bot.get_chat(config.channel).await.ok_or_log();
        match (me, channel) {
            (Some(me), Some(channel)) => {
                channel.title().map(|title|{
                    config.sender.send(crate::persistent::DBAction::SetInfo {
                        name: me.username().to_string(), 
                        channel_name: title.to_string(),
                    }).ok_or_log();
                });
            },
            _ => log::error!("Cannot invoke bot/channel name")
        }

        let set_cmd = bot.set_my_commands([
            BotCommand::new("/help", "Помощь"), 
            BotCommand::new("/create", "Создать"), 
            BotCommand::new("/publish", "Опубликовать"), 
            ]).await;
        if let Err(e) = set_cmd {
            log::error!("Error on bot starting: {:?}", e);
            return
        }
        dispatcher.dispatch().await;
    });
    token
}
