use std::sync::Arc;

use teloxide::RequestError;
use teloxide::types::{MessageKind, ForwardedFrom, BotCommand, Me, InlineKeyboardMarkup};
use teloxide::dispatching::UpdateFilterExt;
use teloxide::utils::command::BotCommands;
use teloxide::prelude::DependencyMap;
use teloxide::dptree::Handler;
use super::*;
use super::WrappedBot as WBot;

type MyDialogue = Dialogue<State, MyStorage>;
pub type FSMResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type FSMHandler = Handler<'static, DependencyMap, FSMResult, teloxide::dispatching::DpHandlerDescription>;

const HELP: &str = "Привет! Чтобы создать бота барахолки, используй команду /newbot";
const SEND_TOKEN: &str = "Для начала создай бота с помощью @BotFather.
После создания бота он пришлет тебе токен. Вот этот токен надо прислать сюда.";
const FORWARD: &str = "Отлично! Теперь нужно добавить этого бота в админы канала, чтобы он мог постить туда сообщения.
А чтобы понимать, что это за канал - пересылай сюда любое сообщение оттуда.";
const NOTHING_START: &str = "Сначала используй команду /newbot";
const NEED_FORWARD_FROM_CHANNEL: &str = "Нужно переслать сообщение из канала";
const NOT_FORWARDED_FROM_CHANNEL: &str = "Это не то. Нужно переслать сообщение из канала"; 
const CHOOSE_THE_BOT: &str = "Выбери бота:";
const BOT_IS_READY: &str = "Бот готов. Чтобы запустить бота, используй команду /startbot";
const INVALID_TOKEN: &str = "Токен не подходит. Попробуй сначала";

#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "помощь")]
    Help,
    #[command(description = "создать бота")]
    NewBot,
    #[command(description = "запустить бота")]
    StartBot,
    #[command(description = "мои боты")]
    MyBots,
}

pub fn bot_commands() -> Vec<BotCommand> {
    Command::bot_commands()
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
        .branch( teloxide::handler!(State::WaitToken).endpoint(cmd_on_wait_token) )
        .branch( teloxide::handler!(State::Ready(conf)).endpoint(cmd_on_ready) )
        .endpoint(on_command)
    )
    .branch( teloxide::handler!(State::WaitToken).endpoint(wait_token) )
    .branch( teloxide::handler!(State::WaitForward(token)).endpoint(wait_forward) )
    .endpoint(on_wrong_message)
}

async fn on_command(cmd: Command, bot: WBot, dialogue: MyDialogue, db: DBStorage) -> FSMResult {
    match cmd {
        Command::Help => {
            bot.send_message(dialogue.chat_id(), HELP).await?;
        },
        Command::NewBot => {
            dialogue.update(State::WaitToken).await?;
            bot.send_message(dialogue.chat_id(), SEND_TOKEN).await?;
        },
        Command::StartBot => {
            bot.send_message(dialogue.chat_id(), NOTHING_START).await?;
        },
        Command::MyBots => {
            let bots = db.get_bots(dialogue.chat_id().0).await;
            let buttons = bots.into_iter().map(|(id, name)|{
                vec![InlineKeyboardButton::callback(name, id.to_string())]
            });
            let markup = InlineKeyboardMarkup::new(buttons);
            bot.send_message(dialogue.chat_id(), CHOOSE_THE_BOT).reply_markup(markup).await.unwrap();
        },
    }
    Ok(())
}

async fn cmd_on_wait_token(cmd: Command, bot: WBot, dialogue: MyDialogue, db: DBStorage) -> FSMResult {
    match cmd {
        Command::StartBot => {
            bot.send_message(dialogue.chat_id(), NEED_FORWARD_FROM_CHANNEL).await?;
        },
        _ => return on_command(cmd, bot, dialogue, db).await,
    }
    Ok(())
}

async fn wait_token(msg: Message, bot: WBot, dialogue: MyDialogue) -> FSMResult {
    let token = msg.text().ok_or("Empty token")?;
    dialogue.update(State::WaitForward(token.into())).await?;
    bot.send_message(dialogue.chat_id(), FORWARD).await?;
    Ok(())
}

async fn wait_forward(msg: Message, bot: WBot, dialogue: MyDialogue, token: String) -> FSMResult {
    if let MessageKind::Common(msg) = msg.kind {
        if let Some(forward) = msg.forward {
            if let ForwardedFrom::Chat(chat) = forward.from {
                if chat.is_channel() {
                    let channel_id = chat.id;
                    let admin = msg.from.ok_or("Cannot invoke user for message (admin of bot)")?;
                    let conf = BulletinConfig::new(token, channel_id, vec![]);
                    conf.add_admin(admin.id, make_username(&admin));
                    dialogue.update(State::Ready(conf.into())).await?;
                    bot.send_message(dialogue.chat_id(), BOT_IS_READY).await?;
                    return Ok(())
                }
            }
        } 
    } 
    bot.send_message(dialogue.chat_id(), NOT_FORWARDED_FROM_CHANNEL).await?;
    Ok(())
}

async fn cmd_on_ready(
    cmd: Command, 
    bot: WBot, 
    dialogue: MyDialogue, 
    conf: Arc<BulletinConfig>, 
    sender: Arc<Sender<DBAction>>, 
    db: DBStorage
) -> FSMResult {
    if let Command::StartBot = cmd {
        match check_bot(conf.token.clone()).await {
            Ok(me) => {
                sender.send(DBAction::CreateConfig(conf.clone())).ok_or_log();
                bulletin::start(conf);
                bot.send_message(dialogue.chat_id(), format!("Бот @{} запущен", me.username())).reply_markup(
                    teloxide::types::InlineKeyboardMarkup::default()
                    .append_row(vec![InlineKeyboardButton::url("На чай разработчику", "https://pay.mysbertips.ru/93867309".try_into().unwrap())])
                ).await?;
                dialogue.exit().await?;
            },
            Err(e) => {
                log::error!("cannot create bot: {:?}", e);
                bot.send_message(dialogue.chat_id(), INVALID_TOKEN).await?;
                dialogue.exit().await?;
            },
        }
    } else {
        on_command(cmd, bot, dialogue, db).await?;
    }
    Ok(())
}

async fn check_bot(token: String) -> Result<Me, RequestError> {
    let bot = Bot::new(token);
    bot.get_me().send().await
}

async fn on_wrong_message() -> FSMResult {
    Ok(())
}