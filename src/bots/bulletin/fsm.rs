use std::default;

use teloxide::{dispatching::dialogue, types::{User, ParseMode}};

use super::{*, impls::make_ad_text};
use teloxide::prelude::*;

type MyDialogue = Dialogue<State, Storage>;
type UserId = String;
type Conf = std::sync::Arc<super::bot::Config>;

pub type FSMResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
pub enum State {
    Ready,
    PriceWaitng,
    Filling(Ad),
    Preview(Ad),
    Banned(String),
    WaitForward,
    WaitCause(UserId),
    WaitSelectBanned,
}

impl Default for State {
    fn default() -> Self {
        State::Ready
    }
}

pub fn make_dialogue_handler() -> Handler<'static, DependencyMap, FSMResult, teloxide::dispatching::DpHandlerDescription> {
    dptree::filter_map(Signal::from_update)
    .enter_dialogue::<Signal, Storage, State>()
    .branch(
        dptree::filter_map(Signal::filter_content)
        .branch(teloxide::handler![State::PriceWaitng].endpoint(on_price_waiting))
        .branch(teloxide::handler![State::Filling(ad)].endpoint(on_filling))
        .branch(teloxide::handler![State::WaitForward].endpoint(on_wait_forward))
        .branch(teloxide::handler![State::WaitCause(user_id)].endpoint(on_wait_forward))
        .endpoint(send_need_command)
    ).branch(
        dptree::filter_map(Signal::filter_command)
        .endpoint(on_command)
    ).branch(
        dptree::filter_map(Signal::filter_callback)
        .endpoint(on_callback)
    )
}

async fn send_need_command(
    bot: WBot,
    dialogue: MyDialogue,
) -> FSMResult {
    bot.send_message(dialogue.chat_id(), "Введи какую-нибудь команду (или нажми кнопку)").await?;
    Ok(())
}

async fn on_price_waiting(
    bot: WBot,
    dialogue: MyDialogue,
    (_, content): (User, Content),
) -> FSMResult {
    let msg = if let Some(price) = content.price() {
        dialogue.update(State::Filling(Ad::new(price))).await?;
        "Присылай описание или фотки"
    } else {
        "Это не цена, нужно прислать число"
    };
    bot.send_message(dialogue.chat_id(), msg).await?;
    Ok(())
}

async fn on_filling(
    bot: WBot,
    dialogue: MyDialogue,
    mut ad: Ad,
    (_, content): (User, Content),
) -> FSMResult {
    ad.fill(content);
    dialogue.update(State::Filling(ad)).await?;
    bot.send_message(dialogue.chat_id(), "Присылай описание или фотки").await?;
    Ok(())
}

async fn on_wait_forward() -> FSMResult {
    Ok(())
}

async fn on_wait_cause() -> FSMResult {
    Ok(())
}

async fn on_command(
    bot: WBot,
    dialogue: MyDialogue,
    (user, cmd): (User, Command),
    conf: Conf,
) -> FSMResult {
    match cmd {
        Command::Help => {
            bot.send_message(dialogue.chat_id(), "здесь должен быть хэлп").await?;
        },
        Command::Create => {
            dialogue.update(State::PriceWaitng).await?;
            bot.send_message(dialogue.chat_id(), "засылай цену в рублях одним целым числом").await?;
        },
        Command::Publish => on_publish(bot, user, dialogue).await?,
        Command::Ban => todo!(),
        Command::Unban => todo!(),
    }
    Ok(())
}

async fn on_publish(
    bot: WBot,
    user: User,
    dialogue: MyDialogue
) -> FSMResult {
    match dialogue.get().await?.unwrap_or_default() {
        State::Filling(ad) => {
            bot.send_message(dialogue.chat_id(), make_ad_text(user, &ad)).parse_mode(ParseMode::MarkdownV2).await?;
            dialogue.update(State::Preview(ad)).await?;
        },
        State::Preview(_) => {
            bot.send_message(dialogue.chat_id(), "Посмотри публикацию, если все ок - жми Да").await?;
        },
        State::PriceWaitng => {
            bot.send_message(dialogue.chat_id(), "Сначала давай цену").await?;
        },
        _ => {
            bot.send_message(dialogue.chat_id(), "Сначала надо создать публикацию").await?;
        }
    }
    Ok(())
}
async fn on_callback() -> FSMResult {
    Ok(())
}