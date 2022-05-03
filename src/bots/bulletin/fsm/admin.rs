use teloxide::handler;

use super::*;

pub fn process_admin(handler: FSMHandler) -> FSMHandler {
    handler.branch( 
        dptree::filter(filter_admin)
        .branch(
            dptree::filter_map(Signal::filter_admin_action).endpoint(on_action)
        ).branch(
            dptree::filter_map(Signal::filter_content)
            .branch(handler![State::WaitForward].endpoint(on_wait_forward))
            .branch(handler![State::WaitCause(user_id)].endpoint(on_wait_cause))
        )
    )
}

fn filter_admin(signal: Signal, conf: Conf) -> bool {
    conf.is_admin(&signal.user().id)
}

async fn on_action(
    bot: WBot,
    dialogue: MyDialogue,
    action: AdminAction,
    conf: Conf,
) -> FSMResult {
    let chat_id = dialogue.chat_id();
    match action {
        AdminAction::Ban => {
            bot.send_message(chat_id, "пересылай публу от гавнюка").await?;
            dialogue.update(State::WaitForward).await?;
        },
        AdminAction::Unban => {
            let mut markup = InlineKeyboardMarkup::default();
            for (user_id, cause) in conf.banned_users() {
                let name = bot.get_chat_member(conf.channel, user_id).await
                .ok().map( |u|format!("{} {}", u.user.first_name, u.user.last_name.unwrap_or_default() ))
                .unwrap_or(format!("[{}]", user_id));
                let text = format!("{} ({})", name, cause);
                let data = ron::to_string(&CallbackResponse::User(user_id))?;
                log::info!("data: {}", data);
                markup = markup.append_row(vec![InlineKeyboardButton::callback(text, data)]);
            }
            bot.send_message(dialogue.chat_id(), "Выбери, кого амнистировать").reply_markup(markup).await?;
            dialogue.update(State::WaitSelectBanned).await?;
        },
        AdminAction::UserToUnban(user_id) => {
            conf.unban(user_id);
            bot.send_message(dialogue.chat_id(), "Разбанен").await?;
            dialogue.exit().await?;
        },
    }
    Ok(())
}

async fn on_wait_forward(
    bot: WBot,
    dialogue: MyDialogue,
    content: Content
) -> FSMResult {
    if let Some(user) = invoke_author(&content) {
        dialogue.update(State::WaitCause(user.id)).await?;
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
        conf.ban(user_id, text.text);
        bot.send_message(dialogue.chat_id(), "Забанен").await?;
        dialogue.exit().await?;
    } else {
        bot.send_message(dialogue.chat_id(), "Причину укажи просто текстом").await?;
    }
    Ok(())
}

fn invoke_author(content: &Content) -> Option<&User> {
    let text = match content {
        Content::Text(text) => text,
        Content::TextAndPhoto(text, _) => text,
        _ => None?,
    };
    match text.entities.last()?.kind {
        teloxide::types::MessageEntityKind::TextMention{ref user} => Some(user),
        _ => None
    }
}
