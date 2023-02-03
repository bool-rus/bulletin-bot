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
use crate::bots::flags::*;

type MyDialogue = Dialogue<State, MyStorage>;
pub type FSMResult = Result<()>;
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
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "помощь")]
    Help,
    #[command(description = "создать бота")]
    NewBot,
    #[command(description = "мои боты")]
    MyBots,
    #[command(description = "удалить бота")]
    Delete,
    //Теперь команды для гобального админа
    #[command(description = "уведомить пользователей о новых фичах")]
    PublishInfo(String),
}

pub fn bot_commands() -> Vec<BotCommand> {
    Command::bot_commands().into_iter().take(4).collect()
}

use super::flags::Flags;
use crate::persistent::BulletinConfig; //TODO: надо разобраться с наименованиями

#[derive(Clone, Debug)]
pub enum State {
    Start,
    WaitToken,
    WaitForward(String),
    Changing(i64, String), 
    WaitText(i64, String, usize),
    UpdatingToken(i64, String),
    WaitTag(i64, String),
    EditOptions(i64, String, Flags),
}

impl Default for State {
    fn default() -> Self {
        Self::Start
    }
}

pub fn make_dialogue_handler() -> FSMHandler {
    use teloxide::handler;
    use State::*;
    let message_handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<Command>().endpoint(on_command))
        .branch(handler!(WaitToken).endpoint(wait_token) )
        .branch(handler!(WaitForward(token)).endpoint(wait_forward) )
        .branch(handler!(WaitText(bot_id,name,template_id)).endpoint(on_wait_template))
        .branch(handler!(WaitTag(bot_id,name)).endpoint(on_wait_tag))
        .branch(handler!(UpdatingToken(bot_id, name)).endpoint(on_update_token));
    let callback_handler = Update::filter_callback_query()
        .branch(handler!(EditOptions(id,name,flags )).endpoint(on_edit_options))
        .branch(handler!(Changing(id, name)).endpoint(on_changing_callback))
        .endpoint(on_callback);
    dptree::entry().enter_dialogue::<Update, MyStorage, State>()
        .branch(message_handler)
        .branch(callback_handler)
        .endpoint(on_wrong_message)
}

fn markup_load() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("...", CallbackResponse::Nothing.to_msg_text().unwrap())]
    ])
}

fn markup_edit_bot() -> InlineKeyboardMarkup {
    use CallbackResponse::*;
    let callback = InlineKeyboardButton::callback;
    InlineKeyboardMarkup::new(vec![
        vec![callback("Перезапустить",      Restart.to_msg_text().unwrap()      )],
        vec![callback("Изменить тексты",    EditTemplates.to_msg_text().unwrap())],
        vec![callback("Добавить тег",       AddTag.to_msg_text().unwrap()       )],
        vec![callback("Удалить тег",        RemoveTag.to_msg_text().unwrap()    )],
        vec![callback("Обновить токен",     UpdateToken.to_msg_text().unwrap()  )],
        vec![callback("Опции",              Options.to_msg_text().unwrap()      )],
        vec![CONF.tip_button()],
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
            InlineKeyboardButton::callback(text, CallbackResponse::EditTemplate(i).to_msg_text().unwrap()),
        ]
    }).collect();
    InlineKeyboardMarkup::new(btns)
}

fn with_back_button(markup: InlineKeyboardMarkup) -> InlineKeyboardMarkup {
    markup.append_row(vec![
        InlineKeyboardButton::callback("⬅️ Назад", CallbackResponse::Back.to_msg_text().unwrap())
    ])
}

async fn on_wait_tag(bot: WBot, dialogue: MyDialogue, (bot_id, bot_name): (i64, String), db: DBStorage, msg: Message) -> FSMResult {
    let text = msg.text().ok_or(anyhow!("No text on wait text"))?;
    let cb = CallbackResponse::TagToRemove(text.to_owned()).to_msg_text();
    if let Some(err) = CallbackResponse::TagToRemove(text.to_owned()).to_msg_text().err() {
        log::error!("Invalid tag: {}", err);
        bot.send_message(dialogue.chat_id(), "Такой тег не годится, нужен покороче").await?;
        return Ok(())
    }
    log::info!("cb: {:?}", cb);
    dialogue.update(State::Changing(bot_id, bot_name.clone())).await?;
    db.add_tag(bot_id, text.to_string()).await;
    bot.send_message(dialogue.chat_id(), format!("Тег добавлен (для вступления в силу нужен рестарт бота)\nВыбран бот @{}\nЧто будем делать?", bot_name))
        .reply_markup(markup_edit_bot()).await?;
    Ok(())
}

async fn on_update_token(bot: WBot, dialogue: MyDialogue, (bot_id, bot_name): (i64, String), db: DBStorage, msg: Message) -> FSMResult {
    let token = msg.text().ok_or(anyhow!("No text on wait text"))?;
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
    let text = msg.text().ok_or(anyhow!("No text on wait text"))?;
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

fn markup_options(flags: flags::Flags) -> InlineKeyboardMarkup {
    use CallbackResponse::*;
    let callback = InlineKeyboardButton::callback;
    let status = |flag| {
        if flags.check_flag(flag) {
            "✅"
        } else {
            "☑️"
        }
    };
    InlineKeyboardMarkup::new(vec![
        vec![callback(
            format!("Только подписчики канала {}", status(ONLY_SUBSCRIBERS)),
            ToggleOption(ONLY_SUBSCRIBERS).to_msg_text().unwrap()
        )],
        vec![callback(
            format!("Подписка через бота {}", status(APPROVE_SUBSCRIBE)),
            ToggleOption(APPROVE_SUBSCRIBE).to_msg_text().unwrap()
        )],
        vec![callback("OK".to_owned(), Save.to_msg_text().unwrap())],
    ])
}

async fn on_edit_options(bot: WBot, dialogue: MyDialogue, callback: CallbackQuery, db: DBStorage, 
    (bot_id, bot_name, mut flags): (i64, String, flags::Flags)
) -> FSMResult {
    use CallbackResponse::*;
    let data = callback.data.as_ref().ok_or(anyhow!("cannot invoke data from callback"))?;
    let message_id = callback.message.ok_or(anyhow!("cannot invoke message_id from callback"))?.id;
    match CallbackResponse::from_mst_text(data.as_str())? {
        ToggleOption(flag) => {
            flags.toggle_flag(flag);
            dialogue.update(State::EditOptions(bot_id, bot_name, flags)).await?;
            bot.edit_message_reply_markup(dialogue.chat_id(), message_id).reply_markup(markup_options(flags)).await?;
        }
        Save => {
            db.update_flags(bot_id, flags).await;
            dialogue.update(State::Changing(bot_id, bot_name.clone())).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, 
                format!("Настройки обновлены. Для вступления в силу требуется перезагрузка бота.\nВыбран бот @{}\nЧто будем делать?", bot_name)
            ).reply_markup(markup_edit_bot()).await?;
        }
        callback => {
            bail!("invalid callback on edit option state: {:?}", callback)
        }
    }
    Ok(())
}

async fn on_changing_callback(bot: WBot, dialogue: MyDialogue, callback: CallbackQuery, db: DBStorage, started_bots: StartedBots,
    sender: Arc<Sender<DBAction>>, (bot_id, bot_name): (i64, String)
) -> FSMResult {
    use CallbackResponse::*;
    let data = callback.data.as_ref().ok_or(anyhow!("cannot invoke data from callback"))?;
    let message_id = callback.message.ok_or(anyhow!("cannot invoke message_id from callback"))?.id;
    match CallbackResponse::from_mst_text(data.as_str())? {
        Restart => {
            bot.edit_message_reply_markup(dialogue.chat_id(), message_id).reply_markup(markup_load()).await?;
            stop_bot(started_bots.clone(), bot_id).await;
            if let Some(saved_config) = db.get_config(bot_id).await {
                start_bot(bot_id, saved_config, started_bots, sender);
                bot.edit_message_reply_markup(dialogue.chat_id(), message_id).reply_markup(markup_edit_bot()).await?;
            } else {
                bot.send_message(dialogue.chat_id(), "Что-то пошло не так. Бот не найден.").await?;
            }
        },
        EditTemplates => {
            let markup = with_back_button(markup_edit_template(bot_id, &db).await);
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Редактируем тексты для бота @{}",bot_name))
                .reply_markup(markup).await?;
        },
        EditTemplate(template_id) => {
            dialogue.update(State::WaitText(bot_id, bot_name.clone(), template_id)).await?;
            let templates = Template::create(db.get_templates(bot_id).await);
            if template_id >= templates.len() {
                bail!("template_id more than templates count")
            }
            let text = &templates[template_id];

            bot.edit_message_text(dialogue.chat_id(), message_id, 
                format!("Меняем текстовку для @{} (Просто присылай нужную)\nСейчас она выглядит так:\n\n{}", bot_name, text))
                .reply_markup(with_back_button(InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::callback("Сбросить", ResetTemplate.to_msg_text().unwrap())
                ]]))
            ).await?;
        },
        UpdateToken => {
            dialogue.update(State::UpdatingToken(bot_id, bot_name.clone())).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Присылай новый токен для бота {}", bot_name))
                .reply_markup(with_back_button(InlineKeyboardMarkup::default()))
                .await?;
        },
        AddTag => {
            dialogue.update(State::WaitTag(bot_id, bot_name)).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, "Присылай название тега")
                .reply_markup(with_back_button(InlineKeyboardMarkup::default()))
                .await?;
        },
        RemoveTag => {
            let markup: Vec<_> = db.get_tags(bot_id).await.into_iter().map(|name|{
                vec![InlineKeyboardButton::callback(name.clone(), TagToRemove(name).to_msg_text().unwrap())]
            }).collect();
            let markup = with_back_button(InlineKeyboardMarkup::new(markup));
            bot.edit_message_text(dialogue.chat_id(), message_id, "Выбери тег для удаления")
                .reply_markup(markup).await?;
        },
        TagToRemove(tag) => {
            db.delete_tag(bot_id, tag.clone()).await;
            bot.edit_message_text(dialogue.chat_id(), message_id, 
                format!("Тег {} удален! Для вступления изменений в силу требуется перезагрузить бота", tag)
            ).await?;
            bot.send_message(dialogue.chat_id(), format!("Выбран бот @{}\nЧто будем делать?", bot_name))
                .reply_markup(markup_edit_bot()).await?;
        }
        Options => {
            let cfg = db.get_config(bot_id).await.ok_or(anyhow!("bot with id {bot_id} not found"))?;
            dialogue.update(State::EditOptions(bot_id, bot_name.clone(), cfg.flags)).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Можешь настроить опции для бота {bot_name}"))
                .reply_markup(markup_options(cfg.flags)).await?;
        }
        Nothing => {},
        Back => {
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Выбран бот @{}\nЧто будем делать?", bot_name))
                .reply_markup(markup_edit_bot()).await?;
        }
        callback => {
            bail!("invalid callback on changing state: {:?}", callback)
        }
    }
    Ok(())
}

async fn on_callback(bot: WBot, dialogue: MyDialogue, callback: CallbackQuery, db: DBStorage, started_bots: StartedBots) -> FSMResult {
    use CallbackResponse::*;
    let data = callback.data.as_ref().ok_or(anyhow!("cannot invoke data from callback"))?;
    let message_id = callback.message.ok_or(anyhow!("cannot invoke message_id from callback"))?.id;
    match CallbackResponse::from_mst_text(data.as_str())? {
        Select(id) =>  {
            let name = db.get_info(id).await.ok_or(anyhow!("cannot invoke bot_info for id {id}"))?.username;
            dialogue.update(State::Changing(id, name.clone())).await?;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Выбран бот @{}\nЧто будем делать?", name))
                .reply_markup(markup_edit_bot()).await?;
        },
        Remove(id) => {
            let name = db.get_info(id).await.unwrap_or_default().username;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Удаляю бота @{}", name)).await?;
            stop_bot(started_bots, id).await;
            db.delete_config(id).await;
            bot.edit_message_text(dialogue.chat_id(), message_id, format!("Удален бот @{}", name)).await?;
        },
        ResetTemplate => {
            if let Some(State::WaitText(bot_id, name, template_id)) = dialogue.get().await? {
                dialogue.update(State::Changing(bot_id, name.clone())).await?;
                db.delete_template(bot_id, template_id).await;
                bot.send_message(dialogue.chat_id(), format!("Текст сброшен (для вступления в силу нужен рестарт бота)\nВыбран бот @{}\nЧто будем делать?", name))
                    .reply_markup(markup_edit_bot()).await?;
            }
        },
        Back => {
            match dialogue.get_or_default().await? {
                State::Changing(id, name) |
                State::WaitText(id, name, _) |
                State::UpdatingToken(id, name) |
                State::WaitTag(id, name) => {
                    dialogue.update(State::Changing(id, name.clone())).await?;
                    bot.edit_message_text(dialogue.chat_id(), message_id, format!("Выбран бот @{}\nЧто будем делать?", name))
                    .reply_markup(markup_edit_bot()).await?;
                },
                _ => {}
            }
        },
        callback => {
            bot.edit_message_text(dialogue.chat_id(), message_id, "<Неактуально>").await?;
            bail!("invalid callback on common state: {:?}", callback)
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
                    buttons.push(vec![InlineKeyboardButton::callback(name.clone(), CallbackResponse::Select(id).to_msg_text()?)])
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
                    buttons.push(vec![InlineKeyboardButton::callback(name.clone(), CallbackResponse::Remove(id).to_msg_text()?)])
                }
                buttons
            };
            let markup = InlineKeyboardMarkup::new(buttons);
            bot.send_message(dialogue.chat_id(), "Выбери бота для удаления").reply_markup(markup).await.unwrap();
        }
        Command::PublishInfo(text) => if CONF.is_global_admin(dialogue.user_id()) {
            for user_id in db.all_admins().await {
                bot.send_message(user_id, &text).await.ok_or_log();
            }
            bot.send_message(dialogue.chat_id(), "Уведомление разослано").await?;
        },
    }
    Ok(())
}

async fn wait_token(msg: Message, bot: WBot, dialogue: MyDialogue) -> FSMResult {
    let token = msg.text().ok_or(anyhow!("Empty token"))?;

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
                    let admin = msg.from.ok_or(anyhow!("Cannot invoke user for message (admin of bot)"))?;
                    let config = BulletinConfig { token, channel, 
                        admins: vec![(admin.id, make_username(&admin))], templates: vec![], tags: vec![], flags: 0};
                    let id = db.save_config(config.clone()).await?;
                    start_bot(id, config, started_bots, sender);
                    dialogue.exit().await?;
                    bot.send_message(dialogue.chat_id(), "Бот запущен").reply_markup(
                        teloxide::types::InlineKeyboardMarkup::default()
                        .append_row(vec![CONF.tip_button()])
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