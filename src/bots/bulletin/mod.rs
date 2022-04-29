use teloxide::prelude::*;
use super::WrappedBot as WBot;
use entity::*;
use ad::Ad;

type Storage = teloxide::dispatching::dialogue::InMemStorage<fsm::State>;
type Price = u32;

pub mod bot;

mod fsm;
mod entity;
mod impls;
mod ad;