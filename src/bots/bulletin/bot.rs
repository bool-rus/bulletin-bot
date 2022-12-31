use std::sync::Arc;
use crate::impls::LoggableErrorResult;

use super::*;
use teloxide::{types::BotCommand, dispatching::{ShutdownToken, update_listeners::UpdateListener}, error_handlers::ErrorHandler, RequestError, stop::StopToken};

pub fn start(config: Config) -> ShutdownToken {
    let config = Arc::new(config);
    let bot = Bot::new(config.token.as_str());
    let storage = MyStorage::new();
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
        .dependencies(dptree::deps![storage, config.clone()])
        .build();
    let token = dispatcher.shutdown_token();
    tokio::spawn(async move {

        let bot_username = bot.get_me().await.ok_or_log()
            .map(|me|me.username().to_owned())
            .unwrap_or("unknown".to_owned());
        let channel_name = bot.get_chat(config.channel).await.ok_or_log()
            .map(|chat|chat.title().map(ToOwned::to_owned)).flatten()
            .unwrap_or("unknown".to_owned());
        config.sender.send(crate::persistent::DBAction::SetInfo {
            name: bot_username.to_owned(), 
            channel_name,
        }).ok_or_log();

        let set_cmd = bot.set_my_commands([
            BotCommand::new("/help", "Помощь"), 
            BotCommand::new("/create", "Создать"), 
            BotCommand::new("/publish", "Опубликовать"), 
            ]).await;
        if let Err(e) = set_cmd {
            log::error!("Error on bot starting: {:?}", e);
            return
        }
        let mut listener = teloxide::dispatching::update_listeners::polling_default(bot.clone()).await;
        let stop_token = listener.stop_token();
        log::info!("Bot @{} started!", bot_username);
        dispatcher.dispatch_with_listener(
            listener, 
            Arc::new(StoppableErrorHandler(stop_token))
        ).await;
    });
    token
}

struct StoppableErrorHandler(StopToken);

impl ErrorHandler<RequestError> for StoppableErrorHandler  {
    fn handle_error(self: Arc<Self>, error: RequestError) -> futures_util::future::BoxFuture<'static, ()> {
        log::error!("{}", error.to_string());
        if let RequestError::Api(teloxide::ApiError::Unknown(text)) = error {
            if text.to_lowercase() == "unauthorized" {
                self.0.stop();
                log::info!("Bot stopped");
            }
        }
        Box::pin(async {})
    }
}