mod fsm;
mod impls;
mod bots;

use std::{collections::{HashMap, HashSet}, sync::Arc};

use bots::bulletin;
use fsm::*;
use impls::ContextEx;
use tbot::{Bot, contexts::fields::Message, prelude::*, state::StatefulEventLoop};
use tokio::{sync::Mutex, time::sleep};

type UserId = tbot::types::user::Id;
type ChannelId = tbot::types::chat::Id;

const CREATE: &'static str = "Новое объявление";
const PUBLISH: &'static str = "Опубликовать";
const BAN: &'static str = "Забанить";
const UNBAN: &'static str = "Амнистировать";

mod buttons {
    use tbot::types::keyboard::reply::Button;

    use super::*;
    pub const USER_BUTTONS: &[&[Button]] = &[
        &[Button::new(CREATE), Button::new(PUBLISH)],
    ];

    pub const ADMIN_BUTTONS: &[&[Button]] = &[
        &[Button::new(CREATE), Button::new(PUBLISH)],
        &[Button::new(BAN), Button::new(UNBAN)],
    ];
}
use buttons::*;

use crate::impls::LoggableErrorResult;

struct Storage {
    admins: HashSet<UserId>,
    channel: ChannelId,
    conversations: HashMap<UserId, State>,
}
impl Storage {
    pub fn is_admin<T: Message>(&self, msg: &T) -> bool {
        let chat = msg.chat();
        match chat.kind {
            tbot::types::chat::Kind::Private {..} => {
                self.admins.contains(&chat.id.0.into())
            }
            _ => false
        }
    }
    pub fn unban_all(&mut self) {
        self.conversations.retain(|_,state|!matches!(state, State::Banned(..)))
    }

    pub fn process<T: IncomeMessage>(&mut self, user: UserId, signal: Signal<T>) -> (ChannelId, Response) {
        match signal {
            Signal::Ban | Signal::Unban => if !self.admins.contains(&user) { return  (self.channel, Response::WrongMessage)}
            _ => {}
        }
        let conversation = self.conversations.remove(&user).unwrap_or(State::default());
        let (state, mut response) = conversation.process(signal);
        if !matches!(state, State::Ready) {
            self.conversations.insert(user, state);
        };
        match &mut response {
            Response::Ban(user, cause) => {
                self.conversations.insert(UserId::from(*user),State::Banned(cause.clone()));
            }
            Response::BannedUsers(users) => {
                users.extend(self.conversations.iter().filter_map(|(&id, state)|{
                    if matches!(state, State::Banned(..)) {
                        Some(id.0)
                    } else { None }
                }))
            }
            Response::Unban(id) => {
                self.conversations.remove(&UserId::from(*id));
            }
            _ => {}
        }
        (self.channel, response)
    }
    
}

async fn set_commands(bot: &Bot) {
    use tbot::types::parameters::BotCommand;
    let commands = [
        BotCommand::new("help", "Помощь"),
        BotCommand::new("create", "Начать создание объявления"),
        BotCommand::new("publish", "Опубликовать объявление"),
    ];
    bot.set_my_commands(commands.as_slice()).call().await.ok_or_log();
}

async fn run_bot(bot: Bot, storage: Storage) {
    set_commands(&bot).await;
    let channel = storage.channel;
    let storage = Mutex::new(storage);
    let mut bot = bot.stateful_event_loop(storage);
    init_help(&mut bot);

    bot.commands(vec!["create","publish", "ban", "unban"], move |ctx, storage| async move {
        if is_private(ctx.as_ref()) {
            let user_id = ctx.chat_id().0.into();
            let signal: Signal<()> = match ctx.command.as_str() {
                "create" => Signal::Create,
                "publish" => if validate_user(ctx.clone(), channel).await { Signal::Publish} 
                    else { return },
                "ban" => Signal::Ban,
                "unban" => Signal::Unban,
                _ => unreachable!(),
            };
            let (channel, response) = storage.lock().await.process(user_id, signal);
            impls::do_response(ctx.as_ref(), response, channel).await;
        }
    });
    bot.text(move |ctx, storage| async move {
        if is_private(ctx.as_ref()) {
            let user_id = ctx.chat_id().0.into();
            let text = ctx.text.value.as_str();
            let signal = match text {
                CREATE => Signal::Create,
                PUBLISH => if validate_user(ctx.clone(), channel).await { Signal::Publish} 
                    else { return },
                BAN => Signal::Ban,
                UNBAN => Signal::Unban,
                _ => Signal::Message(ctx.clone()),
            };
            let (channel, response) = storage.lock().await.process(user_id, signal);
            impls::do_response(ctx.as_ref(), response, channel).await;
        }
    });
    bot.photo(|ctx, storage| async move {
        if is_private(ctx.as_ref()) {
            let user = if let Some(u) = ctx.from() { u } else {return};
            let signal = Signal::Message(ctx.clone());
            let (channel, response) = storage.lock().await.process(user.id, signal);
            impls::do_response(ctx.as_ref(), response, channel).await;
        }
    });
    bot.data_callback(|ctx, storage| async move {
        let user = &ctx.from;
        let callback = ron::from_str(ctx.data.as_str());
        match callback {
            Ok(callback) => {
                ctx.notify("Принято").call().await.ok_or_log();
                let (channel, response) = storage.lock().await.process::<()>(user.id, Signal::Select(callback));
                impls::do_response(ctx.as_ref(), response, channel).await;
            },
            Err(e) => {
                log::error!("{:?}", e);
                ctx.notify("Упс... что-то пошло не так").call().await.ok_or_log();
            },
        }
        
    });
    bot.polling().start().await.unwrap();
}

fn init_help(bot: &mut StatefulEventLoop<Mutex<Storage>>) {
    bot.commands(vec!["help", "start"], |ctx, storage| async move {
        let is_admin = storage.lock().await.is_admin(ctx.as_ref());
        let markup = if is_admin { ADMIN_BUTTONS } else { USER_BUTTONS };
        ctx.send_message("").reply_markup(markup).call().await.ok_or_log();
    });
}

async fn validate_user<T: ContextEx>(ctx: Arc<T>, channel: ChannelId) -> bool {
    match ctx.bot().get_chat_member(channel, ctx.chat_id().0.into()).call().await {
        Err(err) => {
            log::error!("some trouble on cheking user is member {:?}", err);
            true
        },
        Ok(member) if !member.status.is_left() && !member.status.is_kicked() => true,
        Ok(member) => {
            ctx.bot().send_message(ctx.chat_id(), "Ты не с нами. Уходи.").call().await.ok_or_log();
            log::warn!("user is not a member: {:?}", member);
            false
        }
    }
}

fn is_private<T: Message>(msg: &T) -> bool {
    match msg.chat().kind {
        tbot::types::chat::Kind::Private {..} => true,
        _ => false
    }
}


#[tokio::main]
async fn main() {
    init_logger();

    // let mut conf = bulletin::Config::new(
    //      "5278794412:AAFqSFgFvU_oO4maxaHsdv0gQFCPtq-ycuw".to_owned(),
    //     teloxide::types::ChatId(-1001657257723),
    // );
    // conf.add_admin(teloxide::types::UserId(212858650));
    bots::father::start("1664451950:AAFKLe7bVhzbjJ-G1aoDubjbNBCRQffntE0".into());
    tokio::signal::ctrl_c().await.expect("Failed to listen for ^C");
    //sleep(std::time::Duration::from_secs(5)).await;
    /* 
    use std::env::var;
    let channel = var("CHANNEL_ID").expect("Please, set env variable CHANNEL_ID")
    .parse::<i64>().expect("CHANNEL_ID must be integer");
    let channel = ChannelId::from(channel);
    let token = var("TELEGRAM_BOT_TOKEN").expect("Please, set env variable TELEGRAM_BOT_TOKEN");
    let admins  = var("ADMIN_IDS").expect("Please, set env variable ADMIN_IDS")
    .split(',').map(|id| UserId::from(
        id.parse::<i64>().expect("ADMIN_IDS must be comma separated integers")
    )).collect();
    let bot = Bot::new(token);
    let conversations = Default::default();
    let storage = Storage {admins, channel, conversations};
    run_bot(bot, storage).await;
    */
}

fn init_logger() {
    use simplelog::*;
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
}
