use std::collections::HashMap;
use std::sync::Mutex;

use strum::EnumCount;
use teloxide::types::{UserId, ChatId, KeyboardButton, ReplyMarkup};

pub struct Config {
    pub token: String, 
    pub admin_ids: Vec<UserId>,
    pub channel: ChatId,
    templates: [String; Template::COUNT],
    banned: Mutex<HashMap<UserId, String>>,
}

impl Config {
    pub fn new(token: String, channel: ChatId) -> Self {
        Self {
            token,
            channel,
            admin_ids: Vec::new(),
            banned: Mutex::new(HashMap::new()),
            templates: Template::default_templates(),
        }
    }
    pub fn keyboard(&self, user_id: UserId) -> ReplyMarkup {
        use super::res::*;
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
    pub fn template(&self, template: Template) -> &str {
        self.templates[template as usize].as_str()
    }
}

#[derive(PartialEq, Hash, Clone, Copy, strum_macros::EnumCount)]
#[repr(usize)]
pub enum Template {
    Help,
    RequestPrice,
    NotAPrice,
    RequestDescription,
}

impl Template {
    fn default_templates() -> [String; Template::COUNT] {
        use Template::*;
        let mut r: [String; Template::COUNT] = Default::default();
        r[Help as usize] = super::res::HELP.into();
        r[RequestPrice as usize]   = "Назови свою цену (число) в рублях".into();
        r[NotAPrice as usize]     = "Это не цена, нужно прислать число".into();
        r[RequestDescription as usize] = "Присылай описание или фотки".into();
        r
    }
}
