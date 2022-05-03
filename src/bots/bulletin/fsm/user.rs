use super::*;

pub fn process_user(handler: FSMHandler) -> FSMHandler {
    handler.branch(
        dptree::filter_map(Signal::filter_content)
        .branch(teloxide::handler![State::PriceWaitng].endpoint(on_price_waiting))
        .branch(teloxide::handler![State::Filling(ad)].endpoint(on_filling))
    ).branch(
        dptree::filter_map(Signal::filter_user_action)
        .endpoint(on_user_action)
    )
}

async fn on_price_waiting(
    bot: WBot,
    dialogue: MyDialogue,
    content: Content,
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
    content: Content,
) -> FSMResult {
    ad.fill(content);
    dialogue.update(State::Filling(ad)).await?;
    bot.send_message(dialogue.chat_id(), "Присылай описание или фотки").await?;
    Ok(())
}

async fn on_user_action(
    bot: WBot,
    dialogue: MyDialogue,
    (user, action): (User, UserAction),
    conf: Conf,
) -> FSMResult {
    let chat_id = dialogue.chat_id();
    let user_id = u64::try_from(chat_id.0)?;
    if let Some(cause) = conf.is_banned(&UserId(user_id)) {
        bot.send_message(chat_id, format!("Ты в бане. Причина: {}", cause)).await?;
        dialogue.exit().await?;
        return Ok(())
    }
    match action {
        UserAction::Help => {
            bot.send_message(chat_id, "здесь должен быть хэлп").await?;
        },
        UserAction::Create => {
            dialogue.update(State::PriceWaitng).await?;
            bot.send_message(chat_id, "засылай цену в рублях одним целым числом").await?;
        },
        UserAction::Publish => on_publish(bot, user, dialogue).await?,
        UserAction::Yes => if let State::Preview(ad) = dialogue.get_or_default().await? {
            let msgs: Vec<_> = send_ad(bot.clone(), conf.channel, &user, &ad).await?.into_iter().map(|m|m.id).collect();
            bot.forward_message(chat_id, conf.channel, msgs[0]).await?;
            let data = ron::to_string(&CallbackResponse::Remove(msgs))?;
            bot.send_message(chat_id, "Объявление опубликовано").parse_mode(ParseMode::MarkdownV2)
            .reply_markup(InlineKeyboardMarkup::default().append_row(vec![
                InlineKeyboardButton::callback("Снять с публикации".to_owned(), data),
            ])).await?;
            dialogue.exit().await?;
        },
        UserAction::No => if let State::Preview(ad) = dialogue.get_or_default().await? {
            dialogue.update(State::Filling(ad)).await?;
            bot.send_message(chat_id, "можешь поправить публикацию").await?;
        },
        UserAction::Remove(msgs) => for msg in msgs {
            bot.delete_message(conf.channel, msg).await?;
        },
    }
    Ok(())
}

async fn on_publish(
    bot: WBot,
    user: User,
    dialogue: MyDialogue
) -> FSMResult {
    let chat_id = dialogue.chat_id();
    match dialogue.get().await?.unwrap_or_default() {
        State::Filling(ad) => {
            send_ad(bot.clone(), chat_id, &user, &ad).await?;
            bot.send_message(chat_id, "Все верно?")
            .reply_markup(InlineKeyboardMarkup::default().append_row(vec![
                InlineKeyboardButton::callback("Да".to_owned(), ron::to_string(&CallbackResponse::Yes)?),
                InlineKeyboardButton::callback("Нет".to_owned(), ron::to_string(&CallbackResponse::No)?),
            ])).await?;
            dialogue.update(State::Preview(ad)).await?;
        },
        State::Preview(_) => {
            bot.send_message(chat_id, "Посмотри публикацию, если все ок - жми Да").await?;
        },
        State::PriceWaitng => {
            bot.send_message(chat_id, "Сначала давай цену").await?;
        },
        _ => {
            bot.send_message(chat_id, "Сначала надо создать публикацию").await?;
        }
    }
    Ok(())
}
