use teloxide::handler;

use crate::persistent::BanInfo;

use super::*;

pub fn process_admin(handler: FSMHandler) -> FSMHandler {
    handler.branch(
        dptree::filter_map(Signal::filter_admin_action).endpoint(on_action)
    ).branch(
        dptree::filter_map(Signal::filter_content)
        .branch(handler![State::WaitForward].endpoint(on_wait_forward))
        .branch(handler![State::WaitCause(user_id)].endpoint(on_wait_cause))
        .branch(handler![State::WaitForwardForAdmin].endpoint(on_wait_forward_for_admin))
    )
}

async fn on_action(
    bot: WBot,
    dialogue: MyDialogue,
    action: AdminAction,
    conf: Conf,
    upd: Update,
) -> FSMResult {
    let chat_id = dialogue.chat_id();
    use AdminAction::*;
    match action {
        Ban => {
            bot.send_message(chat_id, "пересылай публикацию злодея").await?;
            dialogue.update(State::WaitForward).await?;
        },
        Unban => {
            let mut markup = InlineKeyboardMarkup::default();
            for (user_id, BanInfo {name, cause}) in conf.banned_users() {
                let text = format!("{name} ({cause})");
                let data = CallbackResponse::User(user_id).to_msg_text()?;
                markup = markup.append_row(vec![InlineKeyboardButton::callback(text, data)]);
            }
            bot.send_message(dialogue.chat_id(), "Выбери, кого амнистировать").reply_markup(markup).await?;
            dialogue.update(State::WaitSelectBanned).await?;
        },
        UserToUnban(user_id) => {
            conf.unban(user_id);
            bot.send_message(dialogue.chat_id(), "Разбанен").await?;
            dialogue.exit().await?;
        },
        AddAdmin => {
            bot.send_message(dialogue.chat_id(), "Пересылай сообщение от человека - сделаем его админом").await?;
            dialogue.update(State::WaitForwardForAdmin).await?;
        },
        RemoveAdmin => {
            let markup = InlineKeyboardMarkup::default().inline_keyboard(conf.admins().into_iter()
                .map(|(id, name)|InlineKeyboardButton::callback(
                    name, 
                    CallbackResponse::AdminToRemove(id).to_msg_text().unwrap())
                )
                .map(|btn|vec![btn])
            );
            bot.send_message(dialogue.chat_id(), "Выбери, кого разжаловать").reply_markup(markup).await?;
        }
        AdminToRemove(u) => {
            if let Some(name) = conf.remove_admin(u) {
                bot.send_message(dialogue.chat_id(), format!("{name} больше не админ")).await?;
            }
        },
        ApproveSubscribe(user_id) => {
            let chat_id = ChatId(user_id.0 as i64);
            bot.approve_chat_join_request(conf.channel, user_id).await?;
            bot.send_message(chat_id, conf.template(Template::JoinApproved)).await?;
            update_request_message(bot, upd, true).await?;
        }
        DeclineSubscribe(user_id) => {
            let chat_id = ChatId(user_id.0 as i64);
            bot.decline_chat_join_request(conf.channel, user_id).await?;
            bot.send_message(chat_id, conf.template(Template::JoinDeclined)).await?;
            update_request_message(bot, upd, false).await?;
        }
    }
    Ok(())
}

async fn update_request_message(bot: WBot, upd: Update, approved: bool) -> FSMResult {
    let text = if approved { "Запрос принят" } else { "Запрос отклонен" };
    if let UpdateKind::CallbackQuery(q) = upd.kind {
        let msg = q.message.ok_or(anyhow!["Cannot invoke message from callback query"])?;
        bot.edit_message_text(msg.chat.id, msg.id, text).await?;
    } else {
        bail!("Expects callback query, but not")
    }
    Ok(())
}

async fn on_wait_forward(
    bot: WBot,
    dialogue: MyDialogue,
    content: Content
) -> FSMResult {
    if let Some(user_id) = invoke_author(&content) {
        dialogue.update(State::WaitCause(user_id)).await?;
        bot.send_message(dialogue.chat_id(), "Пиши причину").await?;
    } else {
        bot.send_message(dialogue.chat_id(), "Это не публикация").await?;
    }
    Ok(())
}

async fn on_wait_cause(
    bot: WBot,
    dialogue: MyDialogue,
    content: Content,
    user_id: UserId,
    conf: Conf,
) -> FSMResult {
    if let Content::Text(text) = content {
        let name = bot.get_chat_member(conf.channel, user_id).await
            .ok().map( |u|format!("{} {}", u.user.first_name, u.user.last_name.unwrap_or_default() ))
            .unwrap_or(format!("[{}]", user_id));
        let info = BanInfo {name, cause: text.text};
        conf.ban(user_id, info);
        bot.send_message(dialogue.chat_id(), "Забанен").await?;
        dialogue.exit().await?;
    } else {
        bot.send_message(dialogue.chat_id(), "Причину укажи просто текстом").await?;
    }
    Ok(())
}

async fn on_wait_forward_for_admin(upd: Update, dialogue: MyDialogue, conf: Conf, bot: WBot) -> FSMResult {
    if let teloxide::types::UpdateKind::Message(msg) = upd.kind {
        if let Some(admin) = msg.forward_from_user() {
            conf.add_admin(admin.id, make_username(admin));
            bot.send_message(dialogue.chat_id(), "Отлично! Новый админ добавлен!").await?;
            return Ok(())
        }
    }
    bot.send_message(dialogue.chat_id(), "Это не то. Нужно переслать любое сообщение от человека, которого ты хочешь сделать админом").await?;
    Ok(())
}
