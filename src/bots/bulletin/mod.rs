use super::*;
use super::WrappedBot as WBot;
use entity::*;
use ad::Ad;

pub use config::Config;
pub use bot::start;
pub use config::Template;
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
