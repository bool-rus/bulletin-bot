use teloxide::{prelude::*, types::{ChatId, InlineKeyboardButton, UserId}};
use crossbeam::channel::Sender;
use crate::persistent::DBAction;
pub mod bulletin;
pub mod father;

pub const TELEGRAM_USER_ID: UserId = UserId(777000);

type WrappedBot = AutoSend<Bot>;

fn make_username(user: &teloxide::types::User) -> String {
    let name = user.first_name.as_str();
    let last_name = user.last_name.as_ref().map(|s|format!(" {}", s)).unwrap_or_default();
    let nick = user.username.as_ref().map(|s|format!(" [@{}]", s)).unwrap_or_default();
    format!("{name}{last_name}{nick}")
}

pub struct GlobalConfig {
    father_channel: ChatId,
    global_admin: UserId,
    tip_button: InlineKeyboardButton,
}