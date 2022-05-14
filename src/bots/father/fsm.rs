use std::sync::Arc;

use teloxide::types::{MessageKind, ForwardedFrom};
use teloxide::dispatching::UpdateFilterExt;
use teloxide::utils::command::BotCommands;
use teloxide::prelude::DependencyMap;
use teloxide::dptree::Handler;
use super::*;
use super::WrappedBot as WBot;

type MyDialogue = Dialogue<State, MyStorage>;
pub type FSMResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type FSMHandler = Handler<'static, DependencyMap, FSMResult, teloxide::dispatching::DpHandlerDescription>;


#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "превед медвед")]
    Help,
    #[command(description = "handle a username.")]
    NewBot,
    Start,
}

#[derive(Clone)]
pub enum State {
    Start,
    WaitToken,
    WaitForward(String),
    Ready(Arc<BulletinConfig>),
}

impl Default for State {
    fn default() -> Self {
        Self::Start
    }
}

pub fn make_dialogue_handler() -> FSMHandler {
    Update::filter_message()
    .enter_dialogue::<Update, MyStorage, State>()
    .branch(
        dptree::entry().filter_command::<Command>()
        .branch( teloxide::handler!(State::Start).endpoint(cmd_on_start) )
        .branch( teloxide::handler!(State::Ready(conf)).endpoint(cmd_on_ready) )
    )
    .branch( teloxide::handler!(State::WaitToken).endpoint(wait_token) )
    .branch( teloxide::handler!(State::WaitForward(token)).endpoint(wait_forward) )
    .endpoint(on_wrong_message)
}

async fn cmd_on_start(cmd: Command, bot: WBot, dialogue: MyDialogue) -> FSMResult {
    match cmd {
        Command::Help => {
            bot.send_message(dialogue.chat_id(), "HELP").await?;
        },
        Command::NewBot => {
            dialogue.update(State::WaitToken).await?;
            bot.send_message(dialogue.chat_id(), "Присылай токен").await?;
        },
        Command::Start => {
            bot.send_message(dialogue.chat_id(), "Надо сначала создать бота").await?;
        },
    }
    Ok(())
}

async fn wait_token(msg: Message, bot: WBot, dialogue: MyDialogue) -> FSMResult {
    let token = msg.text().ok_or("Empty token")?;
    dialogue.update(State::WaitForward(token.into())).await?;
    bot.send_message(dialogue.chat_id(), "Теперь пересылай сообщение из канала").await?;
    Ok(())
}

async fn wait_forward(msg: Message, bot: WBot, dialogue: MyDialogue, token: String) -> FSMResult {
    if let MessageKind::Common(msg) = msg.kind {
        if let Some(forward) = msg.forward {
            if let ForwardedFrom::Chat(chat) = forward.from {
                if chat.is_channel() {
                    let channel_id = chat.id;
                    let mut conf = BulletinConfig::new(token, channel_id);
                    conf.add_admin(msg.from.unwrap().id);
                    dialogue.update(State::Ready(conf.into())).await?;
                    bot.send_message(dialogue.chat_id(), "Бот готов").await?;
                }
            }
        } 
    } else {
        bot.send_message(dialogue.chat_id(), "Это не то. Нужно переслать сообщение из канала").await?;
    }
    Ok(())
}

async fn cmd_on_ready(cmd: Command, bot: WBot, dialogue: MyDialogue, conf: Arc<BulletinConfig>, db: Arc<crate::pers::Storage>) -> FSMResult {
    if let Command::Start = cmd {
        db.create_config(conf.token.clone(), conf.channel.0, dialogue.chat_id().0).await;
        super::bulletin::start(conf);
        bot.send_message(dialogue.chat_id(), "Бот запущен").await?;
    }
    Ok(())
}


async fn on_wrong_message() -> FSMResult {
    Ok(())
}