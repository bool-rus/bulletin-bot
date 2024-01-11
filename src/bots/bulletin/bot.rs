use std::sync::Arc;
use crate::impls::LoggableErrorResult;

use super::*;
use teloxide::stop::StopToken;
use teloxide::RequestError;
use teloxide::error_handlers::ErrorHandler;
use teloxide::dispatching::update_listeners::UpdateListener;
use teloxide::dispatching::ShutdownToken;
use teloxide::types::BotCommand;

pub fn start(config: Config) -> ShutdownToken {
    let config = Arc::new(config);
    let bot = Bot::new(config.token.as_str()).throttle(Default::default());
    let storage = MyStorage::new();
    let mut dispatcher = Dispatcher::builder(bot.clone(), fsm::make_dialogue_handler())
        .dependencies(dptree::deps![storage, config.clone()])
        .build();
    let token = dispatcher.shutdown_token();
    tokio::spawn(async move {

        let bot_username = bot.get_me().await.ok_or_log()
            .map(|me|me.username().to_owned())
            .unwrap_or("unknown".to_owned());
        let mut channel_name = "unknown".to_owned();
        let mut invite_link = None;
        if let Some(chat) = bot.get_chat(config.channel).await.ok_or_log() {
            if let Some(title) = chat.title() {
                channel_name = title.to_owned();
            }
            invite_link = chat.invite_link().map(|s|s.to_owned());
        }
        config.sender.send(persistent::DBAction::SetInfo( persistent::BotInfo {
            username: bot_username.clone(), 
            channel_name,
            invite_link,
        })).ok_or_log();

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
        if let RequestError::Api(teloxide::ApiError::NotFound) = error {
            self.0.stop();
            log::info!("Bot stopped");
        }
        Box::pin(async {})
    }
}