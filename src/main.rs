mod fsm;
mod impls;

use std::{cell::Cell, collections::{HashMap, HashSet}};

use futures::StreamExt;
use log::{info, error, warn};
use telegram_bot::*;

use fsm::*;

struct Bot {
    api: Api,
    admins: HashSet<UserId>,
    channel: ChannelId,
    conversations: HashMap<UserId, Cell<State>>,
}

impl Bot {
    async fn start(&mut self) {
        info!("Starting bot...");
        let mut stream = self.api.stream();
        while let Some(update) = stream.next().await {
            info!("update: {:?}", update);
            if let Ok(update) = update {
                match update.kind {
                    UpdateKind::Message(msg) | UpdateKind::EditedMessage(msg) => self.on_message(msg).await,
                    UpdateKind::Error(e) => warn!("Received error update: {}", e),
                    UpdateKind::Unknown => warn!("Unknown update kind, {:?}", update),
                    _ => {},
                }
            } else {
                error!("Error to retrieve update: {:?}", update.err().unwrap());
            }
        }
    }
    fn get_conversation(&mut self, user: UserId) -> &Cell<State> {
        self.conversations.entry(user).or_insert(Default::default())
    }
    async fn on_message(&mut self, msg: Message) {
        let user_id;
        let chat = msg.chat.clone();
        if let MessageChat::Private(user) = &chat {
            user_id = user.id;
        } else { return }
        let cell = self.get_conversation(user_id);
        let (state, response) = cell.take().process(msg.into());
        cell.set(state);
        self.do_response(chat, response).await;
    }
    async fn do_response(&mut self, chat: MessageChat, response: Response) {
        let api = &self.api;
        match response {
            Response::FirstCreate => { api.send(chat.text("Сначала скомандуй /create")).await; }
            Response::PriceRequest => { api.send(chat.text("Назови свою цену")).await; }
            Response::NotPrice => { api.send(chat.text("Это не цена")).await; }
            Response::FillRequest => { api.send(chat.text("Присылай описание или фотки")).await; }
            Response::ContinueFilling => { api.send(chat.text("Что-то еще?")).await; }
            Response::WrongMessage => { api.send(chat.text("Что-то не то присылаешь")).await; }
            Response::CannotPublish => { api.send(chat.text("Пока не могу опубликовать")).await; }
            Response::Publish(ad) => { self.publish(ad).await; }
            Response::Ban(user_id, cause) => { 
                self.get_conversation(UserId::new(user_id)).set(State::Banned(cause));
                self.api.send(chat.text("Принято, больше не нахулиганит")).await; 
            }
            Response::Banned(cause) => { api.send(chat.text(format!("Сорян, ты в бане.\nПричина: {}", cause))).await; }
            Response::ForwardMe => { api.send(chat.text("Пересылай объявление с нарушением")).await; }
            Response::SendCause => { api.send(chat.text("Укажи причину бана")).await; }
            Response::Empty => {  }
        }
    }
    async fn publish(&mut self, ad: Ad) {

    }

}

#[tokio::main]
async fn main() {
    init_logger();
    use std::env::var;
    let channel = var("CHANNEL_ID").expect("Please, set env variable CHANNEL_ID")
    .parse::<i64>().expect("CHANNEL_ID must be integer");
    let channel = ChannelId::new(channel);
    let token = var("TELEGRAM_BOT_TOKEN").expect("Please, set env variable TELEGRAM_BOT_TOKEN");
    let admins  = var("ADMIN_IDS").expect("Please, set env variable ADMIN_IDS")
    .split(',').map(|id| UserId::new(
        id.parse::<i64>().expect("ADMIN_IDS must be comma separated integers")
    )).collect();
    let api = Api::new(token);
    let conversations = Default::default();
    Bot {admins, api, channel, conversations}.start().await;
}

fn init_logger() {
    use simplelog::*;
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
}
