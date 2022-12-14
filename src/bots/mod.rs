use std::sync::{Mutex, Arc};
use std::collections::HashMap;

use teloxide::dispatching::ShutdownToken;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, UserId};
use crossbeam::channel::Sender;
use crate::GlobalConfig;
use crate::CONF;
use crate::impls::LoggableErrorResult;
use crate::persistent;
use crate::persistent::DBAction;
pub mod bulletin;
pub mod father;

type DBStorage = std::sync::Arc<crate::persistent::Storage>;
type StartedBots = Arc<Mutex<HashMap<i64, ShutdownToken>>>;

type WrappedBot = Bot;

fn make_username(user: &teloxide::types::User) -> String {
    let name = user.first_name.as_str();
    let last_name = user.last_name.as_ref().map(|s|format!(" {}", s)).unwrap_or_default();
    let nick = user.username.as_ref().map(|s|format!(" [@{}]", s)).unwrap_or_default();
    format!("{name}{last_name}{nick}")
}

pub async fn start() {
    let (sender, configs, storage) = persistent::worker().await;
    let started_bots = configs.into_iter().fold(HashMap::new(),|mut map, (id, conf)|{
        let conf: bulletin::Config = conf.into();
        let receiver = conf.receiver.clone();
        map.insert(id, bulletin::start(conf));
        sender.send(persistent::DBAction::AddListener(id, receiver)).unwrap();
        map
    });
    father::start(
        sender,
        storage.clone(),
        Arc::new(Mutex::new(started_bots))
    ).await.ok_or_log();
    storage.close().await;
}


impl GlobalConfig {
    pub fn is_global_admin(&self, user_id: UserId) -> bool {
        user_id.0 == self.admin
    }
    pub fn tip_button(&self) -> InlineKeyboardButton {
        InlineKeyboardButton::url("На чай разработчику", self.tip_url.as_str().try_into().unwrap())
    }
}

trait GetUserId {
    fn user_id(&self) -> UserId;
}

impl<D,S> GetUserId for Dialogue<D,S> where D: Send + 'static, S: teloxide::dispatching::dialogue::Storage<D> + ?Sized,{
    fn user_id(&self) -> UserId {
        let primitive = self.chat_id().0;
        UserId(primitive as u64)
    }
}