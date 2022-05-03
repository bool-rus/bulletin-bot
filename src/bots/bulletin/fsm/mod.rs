
use teloxide::payloads::SendMessageSetters;
use teloxide::types::{User, ParseMode, InlineKeyboardMarkup, InlineKeyboardButton, UserId};


use self::admin::process_admin;
use self::user::process_user;

use super::impls::send_ad;
use super::*;


type MyDialogue = Dialogue<State, MyStorage>;
type Conf = std::sync::Arc<Config>;

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
}

impl Default for State {
    fn default() -> Self {
        State::Ready
    }
}

pub fn make_dialogue_handler() -> FSMHandler {
    dptree::filter_map(Signal::from_update)
    .enter_dialogue::<Signal, MyStorage, State>()
    .branch(process_user(dptree::entry()))
    .branch(process_admin(dptree::entry()))
    .endpoint(send_need_command)
}


async fn send_need_command(
    bot: WBot,
    dialogue: MyDialogue,
) -> FSMResult {
    bot.send_message(dialogue.chat_id(), "Введи какую-нибудь команду (или нажми кнопку)").await?;
    Ok(())
}
