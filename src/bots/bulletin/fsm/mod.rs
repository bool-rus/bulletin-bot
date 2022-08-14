use teloxide::payloads::{SendMessageSetters, RestrictChatMemberSetters};
use teloxide::types::{ParseMode, InlineKeyboardMarkup, InlineKeyboardButton, UserId, ChatPermissions};


use crate::bots::TELEGRAM_USER_ID;

use self::admin::process_admin;
use self::user::process_user;

use super::config::Template;
use super::impls::send_ad;
use super::*;


type MyDialogue = Dialogue<State, MyStorage>;

mod user;
mod admin;

pub type FSMResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type FSMHandler = Handler<'static, DependencyMap, FSMResult, teloxide::dispatching::DpHandlerDescription>;

#[derive(Clone)]
pub enum State {
    Ready,
    ActionWaiting,
    PriceWaitng(Target),
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
    let group_handler = dptree::filter_map(GroupMessage::from_update)
        .endpoint(on_group_message_with_delete_aliens);
    dptree::entry()
    .branch(dptree::filter(filter_private).chain(private_handler))
    .branch(group_handler)
}


fn filter_admin(upd: Update, conf: Conf) -> bool {
    upd.user().map(|user|conf.is_admin(&user.id)).unwrap_or(false)
}

async fn on_group_message_with_delete_aliens(msg: GroupMessage, bot: WBot, conf: Conf) -> FSMResult {
    let is_alien = if msg.author == TELEGRAM_USER_ID {
        false
    } else {
        let chat_member = bot.get_chat_member(conf.channel, msg.author).await?;
        chat_member.is_left() || chat_member.is_banned()
    };
    if is_alien {
        bot.delete_message(msg.chat_id, msg.id).await?;
        Err("Автор комментария не подписан на канал, комментарий удален")?
    } else {
        on_group_message(msg, bot, conf).await
    }
}

async fn on_group_message(msg: GroupMessage, bot: WBot, conf: Conf) -> FSMResult {
    match msg.kind {
        GroupMessageKind::Comment { thread, replied_author } => if replied_author != msg.author { 
            let chat_id = teloxide::types::ChatId(replied_author.0 as i64);
            let text = conf.template(Template::NewComment);
            let text = impls::make_message_link(text, &msg.url, Some(thread)).unwrap_or(text.into());
            bot.send_message(chat_id, text).parse_mode(ParseMode::MarkdownV2).await?;
        },
        GroupMessageKind::Mute(user_id) => {
            if conf.is_admin(&msg.author) {
                let until = chrono::Utc::now() + chrono::Duration::weeks(2);
                bot.restrict_chat_member(msg.chat_id, user_id, ChatPermissions::empty()).until_date(until).await?;
            } else {
                bot.send_message(msg.chat_id, conf.template(Template::AdminsOnly)).reply_to_message_id(msg.id).await?;
            }
        },
        GroupMessageKind::Dumb => {},
    }
    Ok(())
}

fn filter_private(u: Update) -> bool {
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
