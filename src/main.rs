use std::cell::Cell;
use std::collections::HashMap;
use fsm::{Signal, State};
use impls::Ad;
use tbot::{contexts::methods::ChatMethods, prelude::*};
use tokio::sync::Mutex;
mod fsm;
mod impls;



#[tokio::main]
async fn main() {
    let channel = tbot::types::chat::Id(
        std::env::var("CHANNEL_ID").unwrap().parse::<i64>().unwrap()
    );

    let state = tokio::sync::Mutex::new(HashMap::new());

    let mut bot =  tbot::from_env!("TELEGRAM_BOT_TOKEN").stateful_event_loop(state);

    bot.command("create", move |ctx, states| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        let id = chat.id;
        let response = {
            let mut states = states.lock().await;
            let state = if let Some(s) = states.get(&id) {
                s
            } else {
                states.insert(id, Cell::new(State::<Ad>::default()));
                states.get(&id).unwrap()
            };
            let (new_state, response) = state.take().process::<(),u32>(Signal::Create);
            state.set(new_state);
            response
        };
        impls::send_response(channel, ctx.as_ref(), response).await;
    });
    bot.command("publish", move |ctx, states| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        let id = chat.id;
        let response = {
            let mut states = states.lock().await;
            let state = if let Some(s) = states.get(&id) {
                s
            } else {
                states.insert(id, Cell::new(State::<Ad>::default()));
                states.get(&id).unwrap()
            };
            let (new_state, response) = state.take().process::<(),u32>(Signal::Publish);
            state.set(new_state);
            response
        };
        impls::send_response(channel, ctx.as_ref(), response).await;
    });

    bot.text(move |ctx, states| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        let id = chat.id;
        let response = {
            let mut states = states.lock().await;
            let state = if let Some(s) = states.get(&id) {
                s
            } else {
                states.insert(id, Cell::new(State::<Ad>::default()));
                states.get(&id).unwrap()
            };
            let (new_state, response) = state.take().process(Signal::Fill(ctx.clone()));
            state.set(new_state);
            response
        };
        impls::send_response(channel, ctx.as_ref(), response).await;        
    });

    bot.photo(move |ctx, states| async move {
        let chat = ctx.chat.clone();
        if !chat.kind.is_private() {return}
        let id = chat.id;
        let response = {
            let mut states = states.lock().await;
            let state = if let Some(s) = states.get(&id) {
                s
            } else {
                states.insert(id, Cell::new(State::<Ad>::default()));
                states.get(&id).unwrap()
            };
            let (new_state, response) = state.take().process(Signal::Fill(ctx.clone()));
            state.set(new_state);
            response
        };
        impls::send_response(channel, ctx.as_ref(), response).await;  
    });

    bot.unhandled(|ctx, _| async move {
        if let Some(msg) = ctx.update.clone().message() {
            if !msg.chat.kind.is_private() {return}
            let chat_id = msg.chat.id;
            ctx.bot.send_message(chat_id, "С таким контентом не работаем").call().await;
        }
    });


    bot.polling().start().await.unwrap();
}
