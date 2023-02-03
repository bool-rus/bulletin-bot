use std::collections::HashMap;
use std::sync::Mutex;
use super::CONF;

use strum::EnumCount;
use teloxide::types::{UserId, ChatId, KeyboardButton, ReplyMarkup};
use super::flags::*;

use crate::{persistent::DBAction, impls::LoggableErrorResult, persistent::BulletinConfig};

pub struct Config {
    pub token: String, 
    pub admins: Mutex<HashMap<UserId, String>>,
    pub channel: ChatId,
    pub sender: crossbeam::channel::Sender<DBAction>,
    pub receiver: crossbeam::channel::Receiver<DBAction>,
    pub tags: Vec<String>,
    flags: Flags,
    templates: [String; Template::COUNT],
    banned: Mutex<HashMap<UserId, String>>,
}

impl Config {
    pub fn keyboard(&self, user_id: UserId) -> ReplyMarkup {
        use super::res::*;
        use KeyboardButton as KB;
        let mut keyboard = vec![
            vec![KB::new(CREATE), KB::new(PUBLISH)]
        ];
        if self.is_admin(&user_id) {
            keyboard.push(vec![KB::new(BAN), KB::new(UNBAN)]);
            keyboard.push(vec![KB::new(ADD_ADMIN), KB::new(REMOVE_ADMIN)]);
        }
        ReplyMarkup::keyboard(keyboard)
    }
    pub fn add_admin(&self, user_id: UserId, name: String) {
        self.admins.lock().unwrap().insert(user_id, name.clone());
        self.sender.send(DBAction::AddAdmin(user_id.0 as i64, name)).ok_or_log();
    }
    pub fn remove_admin(&self, user_id: UserId) -> Option<String> {
        self.sender.send(DBAction::RemoveAdmin(user_id.0 as i64)).ok_or_log();
        self.admins.lock().unwrap().remove(&user_id)
    }
    pub fn admins(&self) -> Vec<(UserId, String)> {
        self.admins.lock().unwrap().iter().map(|(k,v)|(*k, v.clone())).collect()
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
        if CONF.is_global_admin(*user_id) {
            true
        } else {
            self.admins.lock().unwrap().contains_key(user_id)
        }
    }
    pub fn template(&self, template: Template) -> &str {
        self.templates[template as usize].as_str()
    }
    pub fn only_subscribers(&self) -> bool {
        self.flags.check_flag(ONLY_SUBSCRIBERS)
    }
    pub fn approve_subscribe(&self) -> bool {
        self.flags.check_flag(APPROVE_SUBSCRIBE)
    }
}

impl From<BulletinConfig> for Config {
    fn from(cfg: BulletinConfig) -> Self {
        let BulletinConfig {token, channel, admins, templates, tags, flags} = cfg;
        let (sender, receiver) = crossbeam::channel::unbounded();
        let admins = admins.into_iter().collect();
        Self {
            token,
            channel,
            sender,
            receiver,
            admins: Mutex::new(admins),
            banned: Mutex::new(HashMap::new()),
            templates: Template::create(templates),
            tags,
            flags,
        }
    }
}

#[derive(PartialEq, Hash, Clone, Copy, strum_macros::EnumCount)]
#[repr(usize)]
pub enum Template {
    Help,
    RequestPrice,
    NotAPrice,
    FillRequest,
    ContinueFilling,
    Published,
    RemoveAd,
    WrongMessage,
    IsAllCorrect,
    CheckPreview,
    FirstCreate,
    AdRemoved,
    CannotRemoveAd,
    NewComment,
    MuteCommand,
    RequestTarget,
    AdminsOnly,
    Currency,
    BuyText,
    SellText,
    AskText,
    RecommendText,
    WantBuy,
    WantSell,
    WantAsk,
    WantRecommend,
    UserIsNotSubscriber,
    SubscribeInfo,
    JoinApproved,
    JoinDeclined,
}

impl Template {
    pub fn create(overrides: Vec<(usize, String)>) -> [String; Template::COUNT] {
        let mut templates = Template::default_templates();
        for (n, text) in overrides {
            if n < Template::COUNT {
                templates[n] = text;
            }
        }
        templates
    }
    fn default_templates() -> [String; Template::COUNT] {
        use Template::*;
        let mut r: [String; Template::COUNT] = Default::default();
        r[Help as usize] = super::res::HELP.into();
        r[RequestPrice as usize]    = "Назови свою цену (число) в рублях".into();
        r[NotAPrice as usize]       = "Это не цена, нужно прислать число".into();
        r[FillRequest as usize]     = "Присылай описание или фотки. Не забудь выбрать теги ниже".into();
        r[ContinueFilling as usize] = "Теперь можешь заменить описание или добавить фото (не более 10)".into();
        r[Published as usize]       = "Объявление опубликовано".into();
        r[RemoveAd as usize]        = "Снять с публикации".into();
        r[WrongMessage as usize]    = "Что-то не то присылаешь".into();
        r[IsAllCorrect as usize]    = "Все верно?".into();
        r[CheckPreview as usize]    = "Посмотри публикацию, если все ок - жми Да".into();
        r[FirstCreate as usize]     = "Сначала нажми кнопку [Создать] или отправь команду /create".into();
        r[AdRemoved as usize]       = "Публикация удалена".into();
        r[CannotRemoveAd as usize]  = "Не удалось удалить публикацию. Возможно, прошло более 48 часов".into();
        r[NewComment as usize]      = "Добавлен новый комментарий".into();
        r[MuteCommand as usize]     = "!mute".into();
        r[RequestTarget as usize]   = "Цель объявления?".into();
        r[AdminsOnly as usize]      = "Хорошая попытка, но так могут только админы".into();
        r[Currency as usize]        = "₽".into();
        r[BuyText as usize]         = "куплю за".into();
        r[SellText as usize]        = "продам за".into();
        r[AskText as usize]         = "вопрос".into();
        r[RecommendText as usize]   = "рекомендация".into();
        r[WantBuy as usize]         = "купить".into();
        r[WantSell as usize]        = "продать".into();
        r[WantAsk as usize]         = "спросить".into();
        r[WantRecommend as usize]   = "порекомендовать".into();
        r[UserIsNotSubscriber as usize] = "Ты не с нами. Уходи".into();
        r[SubscribeInfo as usize]   = "Напиши что-нибудь, сообщение будет переслано админу".into();
        r[JoinApproved as usize]    = "Заявка принята".into();
        r[JoinDeclined as usize]    = "Заявка отклонена".into();
        r
    }
}
