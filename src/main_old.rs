use std::{cell::Cell, collections::HashSet, ops::Deref, sync::Arc};
use std::collections::HashMap;
use fsm::{AsPrice, CanFill, Response, Signal, State};
use impls::Ad;
use tbot::{contexts::methods::ChatMethods, prelude::*, types::Message};
use tokio::sync::Mutex;
mod fsm;
mod impls;

type ChatId = tbot::types::chat::Id;
type UserId = tbot::types::user::Id;

struct GlobalState {
    chats: HashMap<ChatId, Cell<State<Ad>>>,
    admins: HashSet<UserId>,
}

impl GlobalState {
    fn new(admins: HashSet<UserId>) -> Self {
        GlobalState {
            chats: Default::default(),
            admins,
        }
    }
    fn chat_state(&mut self, chat: ChatId) -> &Cell<State<Ad>> {
        let chats = &mut self.chats;
        chats.entry(chat).or_insert(Default::default())
    }
    fn is_admin(&self, user: &UserId) -> bool {
        self.admins.contains(user)
    }
}


async fn update_state<T: CanFill<Ad> + AsPrice<u32>>(state: Arc<Mutex<GlobalState>>, chat: ChatId, signal: Signal<T>) -> Response<Ad> {
    let mut state = state.lock().await;
    let chat_state = state.chat_state(chat);
    let (new_state, response) = chat_state.take().process(signal);
    chat_state.set(new_state);
    response
}

async fn main() {

    let channel = tbot::types::chat::Id(
        std::env::var("CHANNEL_ID").unwrap().parse::<i64>().unwrap()
    );
    let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap();
    let admin_ids = std::env::var("ADMIN_IDS").unwrap().split(',').map(|id|tbot::types::user::Id(
        id.parse::<i64>().unwrap()
    )).collect();

    let state = Mutex::new(GlobalState::new(admin_ids));

    let mut bot = tbot::Bot::new(token).stateful_event_loop(state);

    let command_help = |ctx: Arc<tbot::contexts::Command<tbot::contexts::Text>>, state: Arc<Mutex<GlobalState>>| async move {
        let user: UserId = ctx.chat.id.0.into();
        use tbot::types::keyboard::reply::{Button, RequestKind, RequestPollKind};
        let keyboard: &[&[_]]= &[
            &[Button::new("/create")],
            &[Button::new("/Опубликовать")],
            &[Button::new("Опрос?").request(RequestKind::Poll(RequestPollKind::Any))],
            &[Button::new("Опрос обычный?").request(RequestKind::Poll(RequestPollKind::Regular))],
            &[Button::new("Опрос квиз?").request(RequestKind::Poll(RequestPollKind::Quiz))],
        ];
        let state = state.lock().await;
        let text = if state.is_admin(&user) {r#"
            Привет, босс. Доступные команды:
            /create - запустить создание объявления
            /publish - опубликовать объявление
            /ban запретить кому-то публикацю объявлений
            /unban разбанить пользователя
        "#} else {r#"
            Привет. Доступные команды:
            /create - запустить создание объявления
            /publish - опубликовать объявление
        "#};
        
        let msg = ctx.send_message(text).reply_markup(keyboard);
        msg.call().await;
    };

    bot.command("start", command_help);
    bot.command("help", command_help);

    bot.command("create", move |ctx, state| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        let response = update_state::<()>(state, chat.id, Signal::Create).await;
        impls::send_response(channel, ctx.as_ref(), response).await;
    });
    bot.command("publish", move |ctx, state| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        let response = update_state::<()>(state, chat.id, Signal::Publish).await;
        impls::send_response(channel, ctx.as_ref(), response).await;
    });

    bot.text(move |ctx, state| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        println!("TEXT: {:?}", ctx);
        let response = update_state(state, chat.id, Signal::Fill(ctx.clone())).await;
        impls::send_response(channel, ctx.as_ref(), response).await;        
    });

    bot.photo(move |ctx, state| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        let response = update_state(state, chat.id, Signal::Fill(ctx.clone())).await;
        impls::send_response(channel, ctx.as_ref(), response).await;
    });

    bot.unhandled(|ctx, _| async move {
        if let Some(msg) = ctx.update.clone().message() {
            if !msg.chat.kind.is_private() {return}
            let chat_id = msg.chat.id;
            println!("Unhandled: {:?}", ctx);
            ctx.bot.send_message(chat_id, "С таким контентом не работаем").call().await;
        }
    });


    bot.polling().start().await.unwrap();
}