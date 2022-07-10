use teloxide::prelude::*;
use super::TELEGRAM_USER_ID;
use super::WrappedBot as WBot;
use entity::*;
use ad::Ad;

pub use config::Config;
pub use bot::start;
use super::make_username;

type MyStorage = teloxide::dispatching::dialogue::InMemStorage<fsm::State>;
type Price = u32;
type Conf = std::sync::Arc<Config>;

mod bot;

mod fsm;
mod entity;
mod impls;
mod ad;
mod config;

mod res;
