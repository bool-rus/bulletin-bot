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
use anyhow::{anyhow, Result, bail};

type DBStorage = std::sync::Arc<crate::persistent::Storage>;
type StartedBots = Arc<Mutex<HashMap<i64, ShutdownToken>>>;

type WrappedBot = Bot;

fn make_username(user: &teloxide::types::User) -> String {
    let name = user.first_name.as_str();
    let last_name = user.last_name.as_ref().map(|s|format!(" {}", s)).unwrap_or_default();
    let nick = user.username.as_ref().map(|s|format!(" [@{}]", s)).unwrap_or_default();
    format!("{name}{last_name}{nick}")
}

pub async fn start() -> anyhow::Result<()> {
    let (sender, configs, storage) = persistent::worker().await?;
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
    Ok(())
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

trait CallbackMessage : Sized + serde::Serialize + serde::de::DeserializeOwned {
    fn from_mst_text(s: &str) -> Result<Self> {
        let bytes = base91::slice_decode(s.as_bytes());
        postcard::from_bytes(bytes.as_slice()).map_err(Into::into)
    }
    fn to_msg_text(&self) -> Result<String> {
        //максимальная длина сообщения в Telegram - 64 байта. base91 увеличивает объем на 23% 
        //следовательно, получаем ограничение на 52 байта исходных данных
        let buf = postcard::to_vec::<_, 52>(&self)?;
        let encoded = base91::slice_encode(buf.as_slice());
        String::from_utf8(encoded).map_err(Into::into)
    }
}


pub mod flags {

    pub type Flags = i32;

    pub const ONLY_SUBSCRIBERS: Flags = 0b1;
    pub const APPROVE_SUBSCRIBE: Flags = 0b10;
    pub const WITHOUT_DONATE: Flags = 0b100;

    pub trait FeatureFlags {
        fn check_flag(&self, flag: Flags) -> bool;
        fn toggle_flag(&mut self, flag: Flags);
    }

    impl FeatureFlags for i32 {
        fn check_flag(&self, flag: Flags) -> bool {
            *self | flag == *self
        }

        fn toggle_flag(&mut self, flag: Flags) {
            let result = *self ^ flag;
            *self = result;
        }
    }
    
    #[test]
    fn test_flags() {
        let mut f = 0b1001;
        assert!(f.check_flag(0b1));
        assert!(f.check_flag(0b1000));
        f.toggle_flag(0b1);
        f.toggle_flag(0b100);
        assert_eq!(0b1100, f);
    }
}