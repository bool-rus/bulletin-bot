use std::str::FromStr;

use super::*;
use super::res::*;

use serde::{Serialize, Deserialize};

use teloxide::dispatching::dialogue::GetChatId;
use teloxide::types::{UserId, Update, ChatId, UpdateKind, MessageKind, MediaKind, MediaText, MessageCommon};

type MessageId = i32;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CallbackResponse {
    #[serde(rename="y")]
    Yes,
    #[serde(rename="n")]
    No,
    Target(Target),
    User(UserId),
    Remove(Vec<MessageId>),
    AdminToRemove(UserId),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Target {
    Buy,
    Sell,
    JustAQuestion,
}

#[derive(Clone, Debug)]
pub enum Content {
    Text(MediaText),
    Photo(String),
    TextAndPhoto(MediaText, String),
}
enum Command {
    Help,
    Create,
    Publish,
    Ban,
    Unban,
    AddAdmin,
    RemoveAdmin,
}

#[derive(Clone, Debug)]
pub enum UserAction {
    Help,
    Target(Target),
    Create,
    Publish,
    Yes,
    No,
    Remove(Vec<MessageId>)
}

#[derive(Clone, Debug)]
pub enum AdminAction {
    Ban,
    Unban,
    UserToUnban(UserId),
    AddAdmin,
    RemoveAdmin,
    AdminToRemove(UserId),
}

#[derive(Clone, Debug)]
pub enum SignalKind {
    UserAction(UserAction),
    AdminAction(AdminAction),
    Content(Content),
}

#[derive(Clone)]
pub struct Signal {
    chat_id: ChatId,
    kind: SignalKind,
}

impl Signal {
    pub fn from_update(u: Update) -> Option<Self> {
        match u.kind {
            UpdateKind::Message(msg) | UpdateKind::EditedMessage(msg) => {
                let chat_id = msg.chat.id;
                let kind = if let MessageKind::Common(msg) = msg.kind {
                    let content = media_to_content(msg.media_kind)?;
                    if let Some(cmd) = content.command() {
                        cmd.into()
                    } else {
                        SignalKind::Content(content)
                    }
                } else {
                    return None
                };
                Some(Signal{chat_id, kind})
            },
            UpdateKind::CallbackQuery(q) => {
                let chat_id = q.chat_id()?;
                let data = q.data?;
                match ron::from_str::<CallbackResponse>(data.as_str()) {
                    Ok(response) => Some(Signal{chat_id, kind: response.into()}),
                    Err(e) => {
                        log::error!("cannot parse callback data: {:?}", e);
                        None
                    },
                }
            },
            UpdateKind::Error(e) => { 
                log::error!("Received error: {:?}", e);
                None
            },
            _ => None,
        }
    }
    pub fn filter_user_action(self) -> Option<UserAction> {
        match self.kind {
            SignalKind::UserAction(action) => Some(action),
            _ => None,
        }
    }
    pub fn filter_admin_action(self) -> Option<AdminAction> {
        match self.kind {
            SignalKind::AdminAction(action) => Some(action),
            _ => None
        }
    }
    pub fn filter_content(self) -> Option<Content> {
        match self.kind {
            SignalKind::Content(c) => Some(c),
            _ => None,
        }
    }
}

impl GetChatId for Signal {
    fn chat_id(&self) -> Option<ChatId> {
        Some(self.chat_id)
    }
}

impl FromStr for Command {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "/help" | "/start" => Ok(Self::Help),
            "/create" | CREATE => Ok(Self::Create),
            "/publish" | PUBLISH => Ok(Self::Publish),
            "/ban" | BAN => Ok(Self::Ban),
            "/unban" | UNBAN => Ok(Self::Unban),
            ADD_ADMIN => Ok(Self::AddAdmin),
            REMOVE_ADMIN => Ok(Self::RemoveAdmin),
            _ => Err(())
        }
    }
}

impl Into<SignalKind> for Command {
    fn into(self) -> SignalKind {
        use SignalKind as SK;
        match self {
            Command::Ban => SK::AdminAction(AdminAction::Ban),
            Command::Unban => SK::AdminAction(AdminAction::Unban),
            Command::AddAdmin => SK::AdminAction(AdminAction::AddAdmin),
            Command::RemoveAdmin => SK::AdminAction(AdminAction::RemoveAdmin),
            Command::Help => SK::UserAction(UserAction::Help),
            Command::Create => SK::UserAction(UserAction::Create),
            Command::Publish => SK::UserAction(UserAction::Publish),
        }
    }
}

impl Into<SignalKind> for CallbackResponse {
    fn into(self) -> SignalKind {
        use SignalKind as SK;
        match self {
            CallbackResponse::Yes => SK::UserAction(UserAction::Yes),
            CallbackResponse::No => SK::UserAction(UserAction::No),
            CallbackResponse::User(u) => SK::AdminAction(AdminAction::UserToUnban(u)),
            CallbackResponse::AdminToRemove(u) => SK::AdminAction(AdminAction::AdminToRemove(u)),
            CallbackResponse::Remove(msgs) => SK::UserAction(UserAction::Remove(msgs)),
            CallbackResponse::Target(target) => SK::UserAction(UserAction::Target(target)),
        }
    }
}

impl Content {
    pub fn price(&self) -> Option<Price> {
        match self {
            Self::Text(txt) => {
                txt.text.parse().ok()
            },
            _ => None
        }
    }
    fn command(&self) -> Option<Command> {
        match self {
            Self::Text(txt) => {
                Command::from_str(txt.text.as_str()).ok()
            }
            _ => None
        }
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.text.as_str()),
            _ => None,
        }
    }
}

pub fn media_to_content(media: MediaKind) -> Option<Content> {
    let content = match media {
        MediaKind::Photo(mut photo) => {
            photo.photo.sort_unstable_by_key(|size|size.height);
            let best_size = photo.photo.last()?.file_id.clone();
            let entities = photo.caption_entities;
            match photo.caption {
                Some(text) => Content::TextAndPhoto(MediaText { text, entities }, best_size),
                None => Content::Photo(best_size),
            }
        },
        MediaKind::Text(m) => Content::Text(m),
        _ => return None
    };
    Some(content)
}

pub fn invoke_author(content: &Content) -> Option<UserId> {
    let text = match content {
        Content::Text(text) => text,
        Content::TextAndPhoto(text, _) => text,
        _ => None?,
    };
    match text.entities.first()?.kind {
        teloxide::types::MessageEntityKind::TextLink {ref url} => {
            if let Some(user_id) = url.query().map(|q|q.parse().ok()).flatten() {
                return Some(UserId(user_id));
            }
        }
        _ => {}
    }
    //легаси, через время удалить
    log::warn!("cannot invoke author: {:?}", text);
    match text.entities.last()?.kind {
        teloxide::types::MessageEntityKind::TextMention{ref user} => Some(user.id),
        _ => None
    }
}

#[derive(Clone, Debug)]
pub struct GroupMessage {
    pub id: i32,
    pub chat_id: ChatId,
    pub sender_chat_id: Option<ChatId>,
    pub url: String,
    pub author: UserId,
    pub kind: GroupMessageKind,
}
#[derive(Clone, Debug)]
pub enum GroupMessageKind {
    Comment {thread: i32, replied_author: UserId},
    Mute(UserId),
    Dumb,
}

impl GroupMessage {
    pub fn from_update(u: Update, conf: Conf) -> Option<Self> {
        match u.kind {
            UpdateKind::Message(msg) => Self::from_message(msg, conf),
            _ => None
        }
    }
    pub fn from_message(msg: Message, conf: Conf) -> Option<Self> {
        let url = msg.url()?.to_string();
        let chat_id = msg.chat.id;
        let id = msg.id;
        if let MessageKind::Common(MessageCommon {from, reply_to_message, media_kind, sender_chat, ..}) = msg.kind {
            let sender_chat_id = sender_chat.map(|chat|chat.id);
            let author = from?.id;
            let kind = if let Some(reply_to_message) = reply_to_message {
                let thread = reply_to_message.id;
                let content = media_to_content(media_kind)?;
                if let MessageKind::Common(MessageCommon{from, media_kind, ..}) = reply_to_message.kind {
                    let replied_author = from?.id;
                    let replied_content = media_to_content(media_kind)?;
                    if replied_author == TELEGRAM_USER_ID { 
                        let replied_author = invoke_author(&replied_content)?;
                        GroupMessageKind::Comment { thread, replied_author}
                    } else if content.text()?.to_lowercase() == conf.template(config::Template::MuteCommand).to_lowercase() {
                        GroupMessageKind::Mute(replied_author)
                    } else {
                        None?
                    }
                } else {
                    log::error!("cannot invoke replied message: {:?}", reply_to_message.kind);
                    None?
                }
            } else {
                GroupMessageKind::Dumb
            };
            Some(Self {id, chat_id, sender_chat_id, url, author, kind})
        } else {
            None
        }
    } 
}
