use std::collections::HashMap;
use std::sync::Mutex;

use teloxide::types::{UserId, ChatId, KeyboardButton, ReplyMarkup};
use super::res::*;

pub struct Config {
    pub token: String, 
    pub admin_ids: Vec<UserId>,
    pub channel: ChatId,
    banned: Mutex<HashMap<UserId, String>>,
}

impl Config {
    pub fn new(token: String, channel: ChatId) -> Self {
        Self {
            token,
            channel,
            admin_ids: Vec::new(),
            banned: Mutex::new(HashMap::new()),
        }
    }
    pub fn keyboard(&self, user_id: UserId) -> ReplyMarkup {
        use KeyboardButton as KB;
        let mut keyboard = vec![
            vec![KB::new(CREATE), KB::new(PUBLISH)]
        ];
        if self.is_admin(&user_id) {
            keyboard.push(
                vec![KB::new(BAN), KB::new(UNBAN)]
            )
        }
        ReplyMarkup::keyboard(keyboard)
    }
    pub fn add_admin(&mut self, user_id: UserId) {
        self.admin_ids.push(user_id);
    }
    pub fn ban(&self, user_id: UserId, cause: String) {
        self.banned.lock().unwrap().insert(user_id, cause);
    }
    pub fn unban(&self, user_id: UserId) {
        self.banned.lock().unwrap().remove(&user_id);
    }
    pub fn banned_users(&self) -> Vec<(UserId, String)> {
        self.banned.lock().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect()
    }
    pub fn is_banned(&self, user_id: &UserId) -> Option<String> {
        self.banned.lock().unwrap().get(user_id).cloned()
    }
    pub fn is_admin(&self, user_id: &UserId) -> bool {
        self.admin_ids.contains(user_id)
    }
}