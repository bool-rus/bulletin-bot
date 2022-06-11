
use teloxide::dispatching::UpdateFilterExt;
use teloxide::payloads::SendMessageSetters;
use teloxide::types::{ParseMode, InlineKeyboardMarkup, InlineKeyboardButton, UserId, MessageKind, MediaKind};


use self::admin::process_admin;
use self::user::process_user;

use super::config::Template;
use super::impls::send_ad;
use super::*;


type MyDialogue = Dialogue<State, MyStorage>;
pub type Conf = std::sync::Arc<Config>;

mod user;
mod admin;

pub type FSMResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type FSMHandler = Handler<'static, DependencyMap, FSMResult, teloxide::dispatching::DpHandlerDescription>;

#[derive(Clone)]
pub enum State {
    Ready,
    PriceWaitng,
    Filling(Ad),
    Preview(Ad),
    WaitForward,
    WaitCause(UserId),
    WaitSelectBanned,
    WaitForwardForAdmin,
}

impl Default for State {
    fn default() -> Self {
        State::Ready
    }
}

pub fn make_dialogue_handler() -> FSMHandler {
    let private_handler = dptree::filter_map(Signal::from_update)
    .enter_dialogue::<Signal, MyStorage, State>()
    .branch(process_user(dptree::entry()))
    .branch(process_admin(dptree::entry()))
    .endpoint(on_wrong_message);
    dptree::entry()
    .branch(dptree::filter(filter_private).chain(private_handler))
    .branch(Update::filter_message().endpoint(on_group_message))
}

async fn on_group_message(msg: Message, bot: WBot, conf: Conf) -> FSMResult {
    let text = conf.template(Template::NewComment);
    let text = impls::make_message_link(text, &msg).unwrap_or(text.into());
    if let MessageKind::Common(msg) = msg.kind {
        if let Some(reply) = msg.reply_to_message {
            if let MessageKind::Common(reply) = reply.kind {
                if let Some(content) = message_to_content(reply) {
                    if let Some(UserId(id)) = invoke_author(&content) {
                        let chat_id = teloxide::types::ChatId(id as i64);
                        bot.send_message(chat_id, text).parse_mode(ParseMode::MarkdownV2).await?;
                    }
                };
            }
        } 
    }
    Ok(())
}


fn invoke_author(content: &Content) -> Option<UserId> {
    let text = match content {
        Content::Text(text) => text,
        Content::TextAndPhoto(text, _) => text,
        _ => None?,
    };
    match text.entities.first()?.kind {
        teloxide::types::MessageEntityKind::TextLink {ref url} => {
            if let Some(user_id) = url.query().map(|q|q.parse().ok()).flatten() {
                return Some(UserId(user_id));
            }
        }
        _ => {}
    }
    //легаси, через время удалить
    log::warn!("cannot invoke author: {:?}", text);
    match text.entities.last()?.kind {
        teloxide::types::MessageEntityKind::TextMention{ref user} => Some(user.id),
        _ => None
    }
}


fn filter_private(u: Update) -> bool{
    u.chat().map(|c|c.is_private()).unwrap_or(false)
}

async fn on_wrong_message(
    bot: WBot,
    dialogue: MyDialogue,
    conf: Conf
) -> FSMResult {
    bot.send_message(dialogue.chat_id(), conf.template(Template::WrongMessage)).await?;
    Ok(())
}
