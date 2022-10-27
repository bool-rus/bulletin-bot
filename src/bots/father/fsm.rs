use std::sync::Arc;
use teloxide::RequestError;
use teloxide::types::{MessageKind, ForwardedFrom, BotCommand, Me, InlineKeyboardMarkup};
use teloxide::dispatching::UpdateFilterExt;
use teloxide::utils::command::BotCommands;
use teloxide::prelude::DependencyMap;
use teloxide::dptree::Handler;
use super::*;
use super::WrappedBot as WBot;
use super::entity::CallbackResponse;
use crate::bots::bulletin::Config as RunnableConfig;

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
    #[command(description = "удалить бота")]
    Delete,
}

pub fn bot_commands() -> Vec<BotCommand> {
    Command::bot_commands()
}

use crate::persistent::BulletinConfig; //TODO: надо разобраться с наименованиями

#[derive(Clone)]
pub enum State {
    Start,
    WaitToken,
    WaitForward(String),
    Ready(BulletinConfig),
    Changing(i64), 
}

impl Default for State {
    fn default() -> Self {
        Self::Start
    }
}

pub fn make_dialogue_handler() -> FSMHandler {
    let message_handler = Update::filter_message()
        .branch(
            dptree::entry().filter_command::<Command>()
            .branch( teloxide::handler!(State::WaitToken).endpoint(cmd_on_wait_token) )
            .branch( teloxide::handler!(State::Ready(conf)).endpoint(cmd_on_ready) )
            .endpoint(on_command)
        )
        .branch( teloxide::handler!(State::WaitToken).endpoint(wait_token) )
        .branch( teloxide::handler!(State::WaitForward(token)).endpoint(wait_forward) );
    let callback_handler = Update::filter_callback_query()
        .endpoint(on_callback);
    dptree::entry().enter_dialogue::<Update, MyStorage, State>()
        .branch(message_handler)
        .branch(callback_handler)
        .endpoint(on_wrong_message)
}

fn markup_load() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("...", CallbackResponse::Nothing.to_string().unwrap())]
    ])
}

fn markup_edit_bot() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("Перезапустить",CallbackResponse::Restart.to_string().unwrap())],
        vec![InlineKeyboardButton::callback("Изменить текст сообщений", "two")]
    ])
}

async fn stop_bot(started_bots: StartedBots, id: i64) {
    let token = started_bots.lock().unwrap().remove(&id);
    if let Some(token) = token {
        if let Ok(shutdown) = token.shutdown() {
            shutdown.await;
        }
    }
}

async fn on_callback(bot: WBot, dialogue: MyDialogue, callback: CallbackQuery, db: DBStorage, started_bots: StartedBots,
    sender: Arc<Sender<DBAction>>) -> FSMResult {
    log::info!("Callback: {:?}", callback);
    let callback_id = callback.id;
    let data = callback.data.as_ref().unwrap();
    let message_id = callback.message.unwrap().id; //todo: unwrap - это плохо
    let callback = CallbackResponse::try_from(data.as_str());
    match callback? {
        CallbackResponse::Nothing => {
            bot.answer_callback_query(callback_id).text("Погоди, дай подумать...").await?;
        }
        CallbackResponse::Select(id, name) =>  {
            dialogue.update(State::Changing(id)).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Выбран бот @{}\nЧто будем делать?", name))
                .reply_markup(markup_edit_bot()).await?;
        },
        CallbackResponse::Restart => {
            if let Some(State::Changing(id)) = dialogue.get().await? {
                bot.edit_message_reply_markup(dialogue.chat_id(), message_id).reply_markup(markup_load()).await?;
                stop_bot(started_bots.clone(), id).await;
                if let Some(saved_config) = db.get_config(id).await {
                    let conf: RunnableConfig = saved_config.into();
                    let receiver = conf.receiver.clone();
                    let token = bulletin::start(conf);
                    sender.send(DBAction::AddListener(id, receiver)).ok_or_log();
                    started_bots.lock().unwrap().insert(id, token);
                    bot.edit_message_reply_markup(dialogue.chat_id(), message_id).reply_markup(markup_edit_bot()).await?;
                } else {
                    bot.send_message(dialogue.chat_id(), "Что-то пошло не так. Бот не найден.").await?;
                }
            } else {
                bot.answer_callback_query(callback_id).text("Неизвестное состояние - нужно выбрать бота через команду /mybots").await?;
            }
        },
        CallbackResponse::Remove(id, name) => {
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Удаляю бота @{}", name)).await?;
            stop_bot(started_bots, id).await;
            db.delete_config(id).await;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Удален бот @{}", name)).await?;
        },
    }
    Ok(())
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
            let buttons: Vec<_> = {
                let mut buttons = Vec::with_capacity(bots.len());
                for (id, name) in bots {
                    buttons.push(vec![InlineKeyboardButton::callback(name.clone(), CallbackResponse::Select(id, name).to_string()?)])
                }
                buttons
            };
            let markup = InlineKeyboardMarkup::new(buttons);
            bot.send_message(dialogue.chat_id(), CHOOSE_THE_BOT).reply_markup(markup).await.unwrap();
        },
        Command::Delete => {
            let bots = db.get_bots(dialogue.chat_id().0).await;
            let buttons: Vec<_> = {
                let mut buttons = Vec::with_capacity(bots.len());
                for (id, name) in bots {
                    buttons.push(vec![InlineKeyboardButton::callback(name.clone(), CallbackResponse::Remove(id, name).to_string()?)])
                }
                buttons
            };
            let markup = InlineKeyboardMarkup::new(buttons);
            bot.send_message(dialogue.chat_id(), "Выбери бота для удаления").reply_markup(markup).await.unwrap();
        }
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
                    let channel = chat.id;
                    let admin = msg.from.ok_or("Cannot invoke user for message (admin of bot)")?;
                    let conf = BulletinConfig { token, channel, admins: vec![(admin.id, make_username(&admin))]};
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
    conf: BulletinConfig, 
    sender: Arc<Sender<DBAction>>, 
    db: DBStorage,
    started_bots: StartedBots,
) -> FSMResult {
    if let Command::StartBot = cmd {
        match check_bot(conf.token.clone()).await {
            Ok(me) => {
                let id = db.save_config(conf.clone()).await;
                let conf: RunnableConfig = conf.into();
                let receiver = conf.receiver.clone();
                sender.send(DBAction::AddListener(id, receiver)).ok_or_log();
                let token = bulletin::start(conf);
                started_bots.lock().unwrap().insert(id, token);
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