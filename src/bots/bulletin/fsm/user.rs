use super::*;
use config::Template as Tpl;
use teloxide::types::MessageId;

const LINE_SIZE: usize = 3;

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
    if let Some(price) = content.price() {
        let ad = Ad::new(target, price);
        let msg = bot.send_message(dialogue.chat_id(), conf.template(Tpl::FillRequest)).await?;
        bot.edit_message_reply_markup(dialogue.chat_id(), msg.id)
            .reply_markup(tags_markup(&ad, conf.tags.as_slice(), msg.id.0)).await?;
        dialogue.update(State::Filling(ad)).await?;
    } else {
        bot.send_message(dialogue.chat_id(), conf.template(Tpl::NotAPrice)).await?;
    };
    Ok(())
}

fn tags_markup(ad: &Ad, tags: &[String], message_id: i32) -> InlineKeyboardMarkup {
    let empty = ["notag".to_owned()];
    let tags = if tags.is_empty() {
        empty.as_slice()
    } else {
        tags
    };
    let (mut btns, line) = tags.iter().map(|name|{
        let name = name.clone();
        if ad.tags.contains(&name) {
            InlineKeyboardButton::callback(format!("✅ {}", name), CallbackResponse::RemoveTag(name, message_id).to_string().unwrap())
        } else {
            InlineKeyboardButton::callback(format!("☑️ {}", name), CallbackResponse::AddTag(name, message_id).to_string().unwrap())
        }
    }).fold((Vec::new(), Vec::with_capacity(LINE_SIZE)), |(mut all, mut line), btn| {
        if line.len() < LINE_SIZE {
            line.push(btn)
        } else {
            all.push(line);
            line = Vec::with_capacity(LINE_SIZE);
            line.push(btn);
        };
        (all, line)
    });
    btns.push(line);
    InlineKeyboardMarkup::new(btns)
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
            .reply_markup(InlineKeyboardMarkup::new( vec![
                vec![
                    callback("купить", CallbackResponse::Target(Target::Buy).to_string().unwrap()),
                    callback("продать", CallbackResponse::Target(Target::Sell).to_string().unwrap()),
                ],
                vec![
                    callback("спросить", CallbackResponse::Target(Target::Ask).to_string().unwrap()),
                    callback("рекомендовать", CallbackResponse::Target(Target::Recommend).to_string().unwrap()),
                ]
            ])).await?;
        },
        UserAction::Publish => on_publish(bot, conf, dialogue).await?,
        UserAction::Yes => if let State::Preview(ad) = dialogue.get_or_default().await? {
            let msgs: Vec<_> = send_ad(bot.clone(), conf.clone(), conf.channel, user_id, &ad).await?;
            dialogue.exit().await?;
            let ids: Vec<_> = msgs.iter().map(|m|m.id.0).collect();
            let data = CallbackResponse::Remove(ids).to_string()?;
            let msg = msgs.first().ok_or("Published msgs is empty".to_owned())?;
            let url = msg.url().map(|u|u.to_string()).unwrap_or_default();
            let text = impls::make_message_link(conf.template(Tpl::Published), &url, None)
            .unwrap_or(conf.template(Tpl::Published).into());
            bot.send_message(chat_id, text).parse_mode(ParseMode::MarkdownV2)
            .reply_markup(InlineKeyboardMarkup::default()
                .append_row(vec![InlineKeyboardButton::callback(conf.template(Tpl::RemoveAd), data)])
                .append_row(vec![CONF.tip_button()])
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
            match target {
                Target::Ask |
                Target::Recommend => {
                    let ad = Ad::new(target, 0);
                    let msg = bot.send_message(dialogue.chat_id(), conf.template(Tpl::FillRequest)).await?;
                    bot.edit_message_reply_markup(dialogue.chat_id(), msg.id)
                        .reply_markup(tags_markup(&ad, &conf.tags, msg.id.0)).await?;
                    dialogue.update(State::Filling(ad)).await?;
                }
                target => {
                    dialogue.update(State::PriceWaitng(target)).await?;
                    bot.send_message(chat_id, conf.template(Tpl::RequestPrice)).await?;
                }
            };
        },
        UserAction::AddTag(tag, message_id) => if let State::Filling(mut ad) = dialogue.get_or_default().await? {
            ad.tags.insert(tag);
            let markup = tags_markup(&ad, &conf.tags, message_id);
            dialogue.update(State::Filling(ad)).await?;
            bot.edit_message_reply_markup(dialogue.chat_id(), MessageId(message_id)).reply_markup(markup).await?;
        },
        UserAction::RemoveTag(tag, message_id) => if let State::Filling(mut ad) = dialogue.get_or_default().await? {
            ad.tags.remove(&tag);
            let markup = tags_markup(&ad, &conf.tags, message_id);
            dialogue.update(State::Filling(ad)).await?;
            bot.edit_message_reply_markup(dialogue.chat_id(), MessageId(message_id)).reply_markup(markup).await?;
        },
    }
    Ok(())
}

async fn delete_msgs(bot: &WBot, ids: Vec<i32>, conf: &Conf) -> FSMResult {
    for id in ids {
        bot.delete_message(conf.channel, MessageId(id)).await?;
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
            if let Err(e) = send_ad(bot.clone(), conf.clone(), chat_id, user_id, &ad).await {
                log::error!("some err on crate ad: {:?}", e);
                bot.send_message(chat_id, format!("Упс, что-то пошло не так: {}", e)).await?;
                return Err(e)
            }
            bot.send_message(chat_id, conf.template(Tpl::IsAllCorrect))
            .reply_markup(InlineKeyboardMarkup::default().append_row(vec![
                InlineKeyboardButton::callback("Да".to_owned(), CallbackResponse::Yes.to_string()?),
                InlineKeyboardButton::callback("Нет".to_owned(), CallbackResponse::No.to_string()?),
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
