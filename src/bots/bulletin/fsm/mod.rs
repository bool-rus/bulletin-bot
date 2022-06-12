
use teloxide::payloads::SendMessageSetters;
use teloxide::types::{ParseMode, InlineKeyboardMarkup, InlineKeyboardButton, UserId};


use crate::bots::TELEGRAM_USER_ID;

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
    .branch(process_admin(dptree::filter(filter_admin)))
    .endpoint(on_wrong_message);
    dptree::entry()
    .branch(dptree::filter(filter_private).chain(private_handler))
    .branch(dptree::filter_map(GroupMessage::from_update).endpoint(on_group_message))
}


fn filter_admin(upd: Update, conf: Conf) -> bool {
    upd.user().map(|user|conf.is_admin(&user.id)).unwrap_or(false)
}


async fn on_group_message(msg: GroupMessage, bot: WBot, conf: Conf) -> FSMResult {
    let text = conf.template(Template::NewComment);
    let text = impls::make_message_link(text, &msg.url, msg.thread).unwrap_or(text.into());
    if msg.replied_author == TELEGRAM_USER_ID {
        if let Some(user_id) = invoke_author(&msg.replied_content) {
            if user_id != msg.author { 
                let chat_id = teloxide::types::ChatId(user_id.0 as i64);
                bot.send_message(chat_id, text).parse_mode(ParseMode::MarkdownV2).await?;
            }
        } 
    }
    Ok(())
}

async fn on_admin_message() {

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
