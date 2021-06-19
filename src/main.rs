mod fsm;
mod impls;

use std::collections::{HashMap, HashSet};

use fsm::*;
use tbot::{Bot, contexts::fields::Message, prelude::*, state::StatefulEventLoop};
use tokio::sync::Mutex;

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


async fn run_bot(bot: Bot, storage: Storage) {
    let storage = Mutex::new(storage);
    let mut bot = bot.stateful_event_loop(storage);
    init_help(&mut bot);
    init_commands(&mut bot);
    bot.text(|ctx, storage| async move {
        if is_private(ctx.as_ref()) {
            let user = if let Some(u) = ctx.from() { u } else {return};
            let text = ctx.text.value.as_str();
            let signal = match text {
                CREATE => Signal::Create,
                PUBLISH => Signal::Publish,
                BAN => Signal::Ban,
                UNBAN => Signal::Unban,
                _ => Signal::Message(ctx.clone()),
            };
            let (channel, response) = storage.lock().await.process(user.id, signal);
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
        let storage = storage.lock().await;
        let is_admin = storage.is_admin(ctx.as_ref());
        let text = "Привет! Воспользуйся кнопками для создания и публикации объявлений";
        let markup = if is_admin { ADMIN_BUTTONS } else { USER_BUTTONS };
        ctx.send_message(text).reply_markup(markup).call().await.ok_or_log();
    });
}

fn init_commands(bot: &mut StatefulEventLoop<Mutex<Storage>>) {
    bot.commands(vec!["create","publish", "ban", "unban"], |ctx, storage| async move {
        if is_private(ctx.as_ref()) {
            let user = if let Some(u) = ctx.from() { u } else {return};
            let signal: Signal<()> = match ctx.command.as_str() {
                "create" => Signal::Create,
                "publish" => Signal::Publish,
                "ban" => Signal::Ban,
                "unban" => Signal::Unban,
                _ => unreachable!(),
            };
            let (channel, response) = storage.lock().await.process(user.id, signal);
            impls::do_response(ctx.as_ref(), response, channel).await;
        }
    });

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
}

fn init_logger() {
    use simplelog::*;
    TermLogger::init(LevelFilter::Warn, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
}
