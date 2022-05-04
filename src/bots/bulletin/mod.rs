use teloxide::prelude::*;
use super::WrappedBot as WBot;
use entity::*;
use ad::Ad;

pub use config::Config;
pub use bot::start;


type MyStorage = teloxide::dispatching::dialogue::InMemStorage<fsm::State>;
type Price = u32;

mod bot;

mod fsm;
mod entity;
mod impls;
mod ad;
mod config;

mod res {
    pub const CREATE: &'static str = "Новое объявление";
    pub const PUBLISH: &'static str = "Опубликовать";
    pub const BAN: &'static str = "Забанить";
    pub const UNBAN: &'static str = "Амнистировать";
}
