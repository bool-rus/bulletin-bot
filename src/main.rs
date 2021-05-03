mod fsm;
mod impls;

use std::{cell::Cell, collections::{HashMap, HashSet}, fmt::format, sync::{Arc}};

use log::{info, error, warn};

use fsm::*;

use tbot::{Bot, contexts::{fields::{Context, Message}, methods::ChatMethods}, state::StatefulEventLoop};
use tokio::sync::Mutex;

type UserId = tbot::types::user::Id;
type ChannelId = tbot::types::chat::Id;

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
        let (state, response) = conversation.process(signal);
        if !matches!(state, State::Ready) {
            self.conversations.insert(user, state);
        };
        match &response {
            Response::Ban(user, cause) => {
                self.conversations.insert(UserId::from(*user),State::Banned(cause.clone()));
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
    bot.text(|msg, storage| async move {
        if is_private(msg.as_ref()) {
            let user = if let Some(u) = msg.from() { u } else {return};
            let signal = Signal::Message(msg.clone());
            let (channel, response) = storage.lock().await.process(user.id, signal);
            impls::do_response(msg, response, channel).await;
        }
    });
    bot.photo(|msg, storage| async move {
        if is_private(msg.as_ref()) {
            let user = if let Some(u) = msg.from() { u } else {return};
            let signal = Signal::Message(msg.clone());
            let (channel, response) = storage.lock().await.process(user.id, signal);
            impls::do_response(msg, response, channel).await;
        }
    });
    bot.polling().start().await.unwrap();
}

fn init_help(bot: &mut StatefulEventLoop<Mutex<Storage>>) {
    bot.commands(vec!["help", "start"], |msg, storage| async move {
        let storage = storage.lock().await;
        let text = if storage.is_admin(msg.as_ref()) {
            r#"Привет, босс. Доступные команды:
            /create - запустить создание объявления
            /publish - опубликовать объявление
            /ban запретить кому-то публикацю объявлений
            /unban разбанить пользователя"#
        } else {
            r#"Привет, босс. Доступные команды:
            /create - запустить создание объявления
            /publish - опубликовать объявление"#
        };
        msg.send_message(text).call().await;
    });
}

fn init_commands(bot: &mut StatefulEventLoop<Mutex<Storage>>) {
    bot.commands(vec!["create","publish"], |msg, storage| async move {
        if is_private(msg.as_ref()) {
            let user = if let Some(u) = msg.from() { u } else {return};
            let signal: Signal<()> = match msg.command.as_str() {
                "create" => Signal::Create,
                "publish" => Signal::Publish,
                _ => unreachable!(),
            };
            let (channel, response) = storage.lock().await.process(user.id, signal);
            warn!("response: {:?}", response);
            impls::do_response(msg, response, channel).await;
        }
    });

    bot.commands(vec!["ban","unban"], |msg, storage| async move {
        let user = if let Some(u) = msg.from() { u } else {return};
        let mut storage = storage.lock().await;
        let signal: Signal<()> = match msg.command.as_str() {
            "ban" => Signal::Ban,
            "unban" => Signal::Unban,
            _ => unreachable!(),
        };
        let (channel, response) = storage.process(user.id, signal);
        impls::do_response(msg, response, channel).await;
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
