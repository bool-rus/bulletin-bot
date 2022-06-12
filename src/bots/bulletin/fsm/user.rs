use super::*;
use config::Template as Tpl;
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
    conf: Conf,
) -> FSMResult {
    let msg = if let Some(price) = content.price() {
        dialogue.update(State::Filling(Ad::new(price))).await?;
        conf.template(Tpl::FillRequest)
    } else {
        conf.template(Tpl::NotAPrice)
    };
    bot.send_message(dialogue.chat_id(), msg).await?;
    Ok(())
}

async fn on_filling(
    bot: WBot,
    dialogue: MyDialogue,
    mut ad: Ad,
    content: Content,
    conf: Conf,
) -> FSMResult {
    ad.fill(content);
    dialogue.update(State::Filling(ad)).await?;
    bot.send_message(dialogue.chat_id(), conf.template(Tpl::ContinueFilling)).await?;
    Ok(())
}

async fn on_user_action(
    upd: Update,
    bot: WBot,
    dialogue: MyDialogue,
    action: UserAction,
    conf: Conf,
) -> FSMResult {
    let chat_id = dialogue.chat_id();
    let user_id = UserId(u64::try_from(chat_id.0)?);
    if let Some(cause) = conf.is_banned(&user_id) {
        bot.send_message(chat_id, format!("Ты в бане. Причина: {}", cause)).await?;
        dialogue.exit().await?;
        return Ok(())
    }
    match action {
        UserAction::Help => {
            bot.send_message(chat_id, conf.template(Tpl::Help)).reply_markup(
                conf.keyboard(user_id)
            ).await?;
        },
        UserAction::Create => {
            dialogue.update(State::PriceWaitng).await?;
            bot.send_message(chat_id, conf.template(Tpl::RequestPrice)).await?;
        },
        UserAction::Publish => on_publish(bot, conf, dialogue).await?,
        UserAction::Yes => if let State::Preview(ad) = dialogue.get_or_default().await? {
            let msgs: Vec<_> = send_ad(bot.clone(), conf.clone(), conf.channel, user_id, &ad).await?.into_iter().map(|m|m.id).collect();
            bot.forward_message(chat_id, conf.channel, msgs[0]).await?;
            let data = ron::to_string(&CallbackResponse::Remove(msgs))?;
            bot.send_message(chat_id, conf.template(Tpl::Published)).parse_mode(ParseMode::MarkdownV2)
            .reply_markup(InlineKeyboardMarkup::default()
                .append_row(vec![InlineKeyboardButton::callback(conf.template(Tpl::RemoveAd), data)])
                .append_row(vec![InlineKeyboardButton::url("На чай разработчику", "https://pay.mysbertips.ru/93867309".try_into().unwrap())])
            ).await?;
            dialogue.exit().await?;
        },
        UserAction::No => if let State::Preview(ad) = dialogue.get_or_default().await? {
            dialogue.update(State::Filling(ad)).await?;
            bot.send_message(chat_id, conf.template(Tpl::ContinueFilling)).await?;
        },
        UserAction::Remove(msgs) => {
            let text = match delete_msgs(&bot, msgs, &conf).await {
                Ok(_) => conf.template(Tpl::AdRemoved),
                Err(e) => {
                    log::error!("Err on remove ad: {:?}", e);
                    conf.template(Tpl::CannotRemoveAd)
                }
            };
            if let teloxide::types::UpdateKind::CallbackQuery(ref q) = upd.kind {
                bot.answer_callback_query(q.id.clone()).text(text).await?;
            };
        },
    }
    Ok(())
}

async fn delete_msgs(bot: &WBot, ids: Vec<i32>, conf: &Conf) -> FSMResult {
    for id in ids {
        bot.delete_message(conf.channel, id).await?;
    }
    Ok(())
}

async fn on_publish(
    bot: WBot,
    conf: Conf,
    dialogue: MyDialogue, 
) -> FSMResult {
    let chat_id = dialogue.chat_id();
    let user_id = UserId(u64::try_from(chat_id.0)?);
    match dialogue.get().await?.unwrap_or_default() {
        State::Filling(ad) => {
            send_ad(bot.clone(), conf.clone(), chat_id, user_id, &ad).await?;
            bot.send_message(chat_id, conf.template(Tpl::IsAllCorrect))
            .reply_markup(InlineKeyboardMarkup::default().append_row(vec![
                InlineKeyboardButton::callback("Да".to_owned(), ron::to_string(&CallbackResponse::Yes)?),
                InlineKeyboardButton::callback("Нет".to_owned(), ron::to_string(&CallbackResponse::No)?),
            ])).await?;
            dialogue.update(State::Preview(ad)).await?;
        },
        State::Preview(_) => {
            bot.send_message(chat_id, conf.template(Tpl::CheckPreview)).await?;
        },
        State::PriceWaitng => {
            bot.send_message(chat_id, conf.template(Tpl::RequestPrice)).await?;
        },
        _ => {
            bot.send_message(chat_id, conf.template(Tpl::FirstCreate)).await?;
        }
    }
    Ok(())
}
