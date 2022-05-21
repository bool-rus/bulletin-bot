use teloxide::prelude::*;
use crossbeam::channel::Sender;
use crate::persistent::DBAction;
pub mod bulletin;
pub mod father;

type WrappedBot = AutoSend<Bot>;

fn make_username(user: &teloxide::types::User) -> String {
    let name = user.first_name.as_str();
    let last_name = user.last_name.as_ref().map(|s|format!(" {}", s)).unwrap_or_default();
    let nick = user.username.as_ref().map(|s|format!(" [@{}]", s)).unwrap_or_default();
    format!("{name}{last_name}{nick}")
}