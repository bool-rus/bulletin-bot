use super::*;
use config::Template as Tpl;
pub fn process_user(handler: FSMHandler) -> FSMHandler {
    handler.branch(
        dptree::filter_map(Signal::filter_content)
        .branch(teloxide::handler![State::PriceWaitng(target)].endpoint(on_price_waiting))
        .branch(teloxide::handler![State::Filling(ad)].endpoint(on_filling))
    ).branch(
        dptree::filter_map(Signal::filter_user_action)
        .endpoint(on_user_action)
    )
}

async fn on_price_waiting(
    bot: WBot,
    dialogue: MyDialogue,
    target: Target,
    content: Content,
    conf: Conf,
) -> FSMResult {
    let msg = if let Some(price) = content.price() {
        dialogue.update(State::Filling(Ad::new(target, price))).await?;
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
            dialogue.update(State::ActionWaiting).await?;
            let callback = InlineKeyboardButton::callback;
            bot.send_message(chat_id, conf.template(Tpl::RequestTarget))
            .reply_markup(InlineKeyboardMarkup::default()
                .append_row(vec![
                    callback(conf.template(Tpl::ToBuy), ron::to_string(&CallbackResponse::Target(Target::Buy)).unwrap()),
                    callback(conf.template(Tpl::ToSell), ron::to_string(&CallbackResponse::Target(Target::Sell)).unwrap()),
                ])
                .append_row(vec![callback(conf.template(Tpl::JustAQuestion), ron::to_string(&CallbackResponse::Target(Target::JustAQuestion)).unwrap() )])
            ).await?;
        },
        UserAction::Publish => on_publish(bot, conf, dialogue).await?,
        UserAction::Yes => if let State::Preview(ad) = dialogue.get_or_default().await? {
            let msgs: Vec<_> = send_ad(bot.clone(), conf.clone(), conf.channel, user_id, &ad).await?;
            dialogue.exit().await?;
            let ids: Vec<_> = msgs.iter().map(|m|m.id).collect();
            let data = ron::to_string(&CallbackResponse::Remove(ids))?;
            let msg = msgs.first().ok_or("Published msgs is empty".to_owned())?;
            let url = msg.url().map(|u|u.to_string()).unwrap_or_default();
            let text = impls::make_message_link(conf.template(Tpl::Published), &url, None)
            .unwrap_or(conf.template(Tpl::Published).into());
            bot.send_message(chat_id, text).parse_mode(ParseMode::MarkdownV2)
            .reply_markup(InlineKeyboardMarkup::default()
                .append_row(vec![InlineKeyboardButton::callback(conf.template(Tpl::RemoveAd), data)])
                .append_row(vec![InlineKeyboardButton::url("На чай разработчику", "https://pay.mysbertips.ru/93867309".try_into().unwrap())])
            ).await?;
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
        UserAction::Target(target) => if let State::ActionWaiting = dialogue.get_or_default().await? {
            let text = match target {
                Target::JustAQuestion => {
                    dialogue.update(State::Filling(Ad::new(target, 0))).await?;
                    conf.template(Tpl::FillRequest)
                }
                target => {
                    dialogue.update(State::PriceWaitng(target)).await?;
                    conf.template(Tpl::RequestPrice)
                }
            };
            bot.send_message(chat_id, text).await?;
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
        State::PriceWaitng(_) => {
            bot.send_message(chat_id, conf.template(Tpl::RequestPrice)).await?;
        },
        _ => {
            bot.send_message(chat_id, conf.template(Tpl::FirstCreate)).await?;
        }
    }
    Ok(())
}
