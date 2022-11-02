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
use crate::bots::bulletin::{Config as RunnableConfig, Template};

type MyDialogue = Dialogue<State, MyStorage>;
pub type FSMResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type FSMHandler = Handler<'static, DependencyMap, FSMResult, teloxide::dispatching::DpHandlerDescription>;

const HELP: &str = "Привет! Чтобы создать бота барахолки, используй команду /newbot";
const SEND_TOKEN: &str = "Для начала создай бота с помощью @BotFather.
После создания бота он пришлет тебе токен. Вот этот токен надо прислать сюда.";
const FORWARD: &str = "Отлично! Теперь нужно добавить этого бота в админы канала, чтобы он мог постить туда сообщения.
А чтобы понимать, что это за канал - пересылай сюда любое сообщение оттуда.";
const NOT_FORWARDED_FROM_CHANNEL: &str = "Это не то. Нужно переслать сообщение из канала"; 
const CHOOSE_THE_BOT: &str = "Выбери бота:";
const INVALID_TOKEN: &str = "Неверный токен. Попробуй другой";

#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "помощь")]
    Help,
    #[command(description = "создать бота")]
    NewBot,
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
    Changing(i64, String), 
    WaitText(i64, String, usize),
    UpdatingToken(i64, String),
    WaitTag(i64, String),
}

impl Default for State {
    fn default() -> Self {
        Self::Start
    }
}

pub fn make_dialogue_handler() -> FSMHandler {
    let message_handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(on_command))
        .branch( teloxide::handler!(State::WaitToken).endpoint(wait_token) )
        .branch( teloxide::handler!(State::WaitForward(token)).endpoint(wait_forward) )
        .branch(teloxide::handler!(State::WaitText(bot_id,name,template_id)).endpoint(on_wait_template))
        .branch(teloxide::handler!(State::WaitTag(bot_id,name)).endpoint(on_wait_tag))
        .branch(teloxide::handler!(State::UpdatingToken(bot_id, name)).endpoint(on_update_token));
    let callback_handler = Update::filter_callback_query()
        .branch(teloxide::handler!(State::Changing(id, name)).endpoint(on_changing_callback))
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
    use CallbackResponse::*;
    let callback = InlineKeyboardButton::callback;
    InlineKeyboardMarkup::new(vec![
        vec![callback("Перезапустить",      Restart.to_string().unwrap()        )],
        vec![callback("Изменить тексты",    EditTemplates.to_string().unwrap()  )],
        vec![callback("Добавить тег",       AddTag.to_string().unwrap()         )],
        vec![callback("Удалить тег",        RemoveTag.to_string().unwrap()      )],
        vec![callback("Обновить токен",     UpdateToken.to_string().unwrap()    )],
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

async fn markup_edit_template(bot_id: i64, db: &DBStorage) -> InlineKeyboardMarkup {
    let overrides = db.get_templates(bot_id).await;
    let btns: Vec<_> = Template::create(overrides).into_iter().enumerate().map(|(i, text)|{
        vec![
            InlineKeyboardButton::callback(text, CallbackResponse::EditTemplate(i).to_string().unwrap()),
        ]
    }).collect();
    InlineKeyboardMarkup::new(btns)
}

async fn on_wait_tag(bot: WBot, dialogue: MyDialogue, (bot_id, bot_name): (i64, String), db: DBStorage, msg: Message) -> FSMResult {
    let text = msg.text().ok_or("No text on wait text")?;
    dialogue.update(State::Changing(bot_id, bot_name.clone())).await?;
    db.add_tag(bot_id, text.to_string()).await;
    bot.send_message(dialogue.chat_id(), format!("Тег добавлен (для вступления в силу нужен рестарт бота)\nВыбран бот @{}\nЧто будем делать?", bot_name))
        .reply_markup(markup_edit_bot()).await?;
    Ok(())
}

async fn on_update_token(bot: WBot, dialogue: MyDialogue, (bot_id, bot_name): (i64, String), db: DBStorage, msg: Message) -> FSMResult {
    let token = msg.text().ok_or("No text on wait text")?;
    db.update_token(bot_id, token.to_owned()).await;
    dialogue.update(State::Changing(bot_id, bot_name.clone())).await?;
    bot.send_message(
        dialogue.chat_id(), 
        format!("Токен обновлен (для вступления в силу нужен рестарт бота)\nВыбран бот @{}\nЧто будем делать?", bot_name)
    ).reply_markup(markup_edit_bot()).await?;
    Ok(())
}

async fn on_wait_template(bot: WBot, dialogue: MyDialogue, 
    (bot_id, name, template_id): (i64, String, usize), 
    msg: Message, db: DBStorage) -> FSMResult {
    let text = msg.text().ok_or("No text on wait text")?;
    dialogue.update(State::Changing(bot_id, name.clone())).await?;
    db.add_template(bot_id, template_id, text.to_string()).await;
    bot.send_message(dialogue.chat_id(), format!("Текст заменен (для вступления в силу нужен рестарт бота)\nВыбран бот @{}\nЧто будем делать?", name))
        .reply_markup(markup_edit_bot()).await?;
    Ok(())
}

fn start_bot(id: i64, config: BulletinConfig, started_bots: StartedBots, sender: Arc<Sender<DBAction>>) {
    let conf: RunnableConfig = config.into();
    let receiver = conf.receiver.clone();
    let token = bulletin::start(conf);
    sender.send(DBAction::AddListener(id, receiver)).ok_or_log();
    started_bots.lock().unwrap().insert(id, token);
}
async fn on_changing_callback(bot: WBot, dialogue: MyDialogue, callback: CallbackQuery, db: DBStorage, started_bots: StartedBots,
    sender: Arc<Sender<DBAction>>, (bot_id, bot_name): (i64, String)
) -> FSMResult {
    let data = callback.data.as_ref().ok_or("cannot invoke data from callback")?;
    let message_id = callback.message.ok_or("cannot invoke message_id from callback")?.id;
    let callback = CallbackResponse::try_from(data.as_str())?;
    match callback {
        CallbackResponse::Restart => {
            bot.edit_message_reply_markup(dialogue.chat_id(), message_id).reply_markup(markup_load()).await?;
            stop_bot(started_bots.clone(), bot_id).await;
            if let Some(saved_config) = db.get_config(bot_id).await {
                start_bot(bot_id, saved_config, started_bots, sender);
                bot.edit_message_reply_markup(dialogue.chat_id(), message_id).reply_markup(markup_edit_bot()).await?;
            } else {
                bot.send_message(dialogue.chat_id(), "Что-то пошло не так. Бот не найден.").await?;
            }
        },
        CallbackResponse::EditTemplates => {
            let markup = markup_edit_template(bot_id, &db).await;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Редактируем тексты для бота @{}",bot_name))
                .reply_markup(markup).await?;
        },
        CallbackResponse::EditTemplate(template_id) => {
            dialogue.update(State::WaitText(bot_id, bot_name.clone(), template_id)).await?;
            let templates = Template::create(db.get_templates(bot_id).await);
            if template_id >= templates.len() {
                Err("template_id more than templates count")?;
            }
            let text = &templates[template_id];
            bot.edit_message_text(dialogue.chat_id(), message_id, 
                format!("Меняем текстовку для @{} (Просто присылай нужную)\nСейчас она выглядит так:\n\n{}", bot_name, text))
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::callback("Сбросить", CallbackResponse::ResetTemplate.to_string().unwrap())
                ]])).await?;
        },
        CallbackResponse::UpdateToken => {
            dialogue.update(State::UpdatingToken(bot_id, bot_name.clone())).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Присылай новый токен для бота {}", bot_name)).await?;
        },
        CallbackResponse::AddTag => {
            dialogue.update(State::WaitTag(bot_id, bot_name)).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, "Присылай название тега").await?;
        },
        CallbackResponse::RemoveTag => {
            let markup: Vec<_> = db.get_tags(bot_id).await.into_iter().map(|name|{
                vec![InlineKeyboardButton::callback(name.clone(), CallbackResponse::TagToRemove(name).to_string().unwrap())]
            }).collect();
            bot.edit_message_text(dialogue.chat_id(), message_id, "Выбери тег для удаления")
                .reply_markup(InlineKeyboardMarkup::new(markup)).await?;
        },
        CallbackResponse::TagToRemove(tag) => {
            db.delete_tag(bot_id, tag.clone()).await;
            bot.edit_message_text(dialogue.chat_id(), message_id, 
                format!("Тег {} удален! Для вступления изменений в силу требуется перезагрузить бота", tag)
            ).await?;
            bot.send_message(dialogue.chat_id(), format!("Выбран бот @{}\nЧто будем делать?", bot_name)).reply_markup(markup_edit_bot()).await?;
        },
        CallbackResponse::Nothing => {},
        callback => {
            Err(format!("invalid callback on changing state: {:?}", callback))?;
        }
    }
    Ok(())
}

async fn on_callback(bot: WBot, dialogue: MyDialogue, callback: CallbackQuery, db: DBStorage, started_bots: StartedBots) -> FSMResult {
    let data = callback.data.as_ref().ok_or("cannot invoke data from callback")?;
    let message_id = callback.message.ok_or("cannot invoke message_id from callback")?.id;
    let callback = CallbackResponse::try_from(data.as_str());
    match callback? {
        CallbackResponse::Select(id, name) =>  {
            dialogue.update(State::Changing(id, name.clone())).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Выбран бот @{}\nЧто будем делать?", name))
                .reply_markup(markup_edit_bot()).await?;
        },
        CallbackResponse::Remove(id, name) => {
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Удаляю бота @{}", name)).await?;
            stop_bot(started_bots, id).await;
            db.delete_config(id).await;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Удален бот @{}", name)).await?;
        },
        CallbackResponse::ResetTemplate => {
            if let Some(State::WaitText(bot_id, name, template_id)) = dialogue.get().await? {
                dialogue.update(State::Changing(bot_id, name.clone())).await?;
                db.delete_template(bot_id, template_id).await;
                bot.send_message(dialogue.chat_id(), format!("Текст сброшен (для вступления в силу нужен рестарт бота)\nВыбран бот @{}\nЧто будем делать?", name))
                    .reply_markup(markup_edit_bot()).await?;
            }
        },
        callback => {
            Err(format!("invalid callback on common state: {:?}", callback))?;
        }
    }
    Ok(())
}


async fn on_command(cmd: Command, bot: WBot, dialogue: MyDialogue, db: DBStorage) -> FSMResult {
    if !matches!(cmd, Command::Help) {
        dialogue.exit().await?;
    }
    match cmd {
        Command::Help => {
            bot.send_message(dialogue.chat_id(), HELP).await?;
        },
        Command::NewBot => {
            dialogue.update(State::WaitToken).await?;
            bot.send_message(dialogue.chat_id(), SEND_TOKEN).await?;
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

async fn wait_token(msg: Message, bot: WBot, dialogue: MyDialogue) -> FSMResult {
    let token = msg.text().ok_or("Empty token")?;

    match check_bot(token).await {
        Ok(me) => {
            let name = me.username();
            dialogue.update(State::WaitForward(token.into())).await?;
            bot.send_message(dialogue.chat_id(), format!("Получен токен для бота @{}\n{}", name, FORWARD)).await?;
        },
        Err(e) => {
            log::error!("cannot create bot (maybe bad token): {:?}", e);
            bot.send_message(dialogue.chat_id(), INVALID_TOKEN).await?;
        },
    }
    Ok(())
}

async fn wait_forward(msg: Message, bot: WBot, dialogue: MyDialogue, token: String, db: DBStorage, started_bots: StartedBots, sender: Arc<Sender<DBAction>>) -> FSMResult {
    if let MessageKind::Common(msg) = msg.kind {
        if let Some(forward) = msg.forward {
            if let ForwardedFrom::Chat(chat) = forward.from {
                if chat.is_channel() {
                    let channel = chat.id;
                    let admin = msg.from.ok_or("Cannot invoke user for message (admin of bot)")?;
                    let config = BulletinConfig { token, channel, 
                        admins: vec![(admin.id, make_username(&admin))], templates: vec![], tags: vec![]};
                    let id = db.save_config(config.clone()).await;
                    start_bot(id, config, started_bots, sender);
                    dialogue.exit().await?;
                    bot.send_message(dialogue.chat_id(), "Бот запущен").reply_markup(
                        teloxide::types::InlineKeyboardMarkup::default()
                        .append_row(vec![InlineKeyboardButton::url("На чай разработчику", "https://pay.mysbertips.ru/93867309".try_into().unwrap())])
                    ).await?;
                    return Ok(())
                }
            }
        } 
    } 
    bot.send_message(dialogue.chat_id(), NOT_FORWARDED_FROM_CHANNEL).await?;
    Ok(())
}

async fn check_bot<S: Into<String>>(token: S) -> Result<Me, RequestError> {
    let bot = Bot::new(token);
    bot.get_me().send().await
}

async fn on_wrong_message() -> FSMResult {
    Ok(())
}