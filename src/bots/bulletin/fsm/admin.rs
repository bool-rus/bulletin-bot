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
            .branch(handler![State::WaitCause(user)].endpoint(on_wait_cause))
        )
    )
}

fn filter_admin(signal: Signal, conf: Conf) -> bool {
    conf.admin_ids.contains(&signal.user().id)
}

async fn on_action(
    bot: WBot,
    dialogue: MyDialogue,
    action: AdminAction,
) -> FSMResult {
    let chat_id = dialogue.chat_id();
    match action {
        AdminAction::Ban => {
            bot.send_message(chat_id, "пересылай публу от гавнюка").await?;
            dialogue.update(State::WaitForward).await?;
        },
        AdminAction::Unban => todo!(),
        AdminAction::UserToUnban(user) => todo!(),
    }
    bot.send_message(dialogue.chat_id(), format!("action: {:?}", action)).await?;
    Ok(())
}

async fn on_wait_forward(
    bot: WBot,
    dialogue: MyDialogue,
    (_, content): (User, Content)
) -> FSMResult {
    todo!()
}

async fn on_wait_cause() -> FSMResult {todo!()}
