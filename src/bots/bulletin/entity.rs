use std::str::FromStr;
use super::*;
use super::res::*;

use serde::{Serialize, Deserialize};

use teloxide::dispatching::dialogue::GetChatId;
use teloxide::types::{UserId, Update, ChatId, UpdateKind, MessageKind, MediaKind, User, MediaText, MessageCommon};

type MessageId = i32;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CallbackResponse {
    #[serde(rename="y")]
    Yes,
    #[serde(rename="n")]
    No,
    User(UserId),
    Remove(Vec<MessageId>),
    AdminToRemove(UserId),
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
    user: User,
    kind: SignalKind,
}

impl Signal {
    pub fn from_update(u: Update) -> Option<Self> {
        let user;
        match u.kind {
            UpdateKind::Message(msg) | UpdateKind::EditedMessage(msg) => {
                let chat_id = msg.chat.id;
                let kind = if let MessageKind::Common(msg) = msg.kind {
                    user = msg.from.as_ref().cloned()?;
                    let content = media_to_content(msg.media_kind)?;
                    if let Some(cmd) = content.command() {
                        cmd.into()
                    } else {
                        SignalKind::Content(content)
                    }
                } else {
                    return None
                };
                Some(Signal{chat_id, user, kind})
            },
            UpdateKind::CallbackQuery(q) => {
                let chat_id = q.chat_id()?;
                let user = q.from;
                let data = q.data?;
                match ron::from_str::<CallbackResponse>(data.as_str()) {
                    Ok(response) => Some(Signal{chat_id, user, kind: response.into()}),
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
    pub fn user(&self) -> &User {
        &self.user
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

#[derive(Clone)]
pub struct GroupMessage {
    pub chat_id: ChatId,
    pub url: String,
    pub thread: i32,
    pub author: UserId,
    pub content: Content,
    pub replied_author: UserId,
    pub replied_content: Content,
}

impl GroupMessage {
    pub fn from_update(u: Update) -> Option<Self> {
        match u.kind {
            UpdateKind::Message(msg) => Self::from_message(msg),
            _ => None
        }
    }
    pub fn from_message(msg: Message) -> Option<Self> {
        let url = msg.url()?.to_string();
        let chat_id = msg.chat.id;
        if let MessageKind::Common(MessageCommon {from, reply_to_message, media_kind, ..}) = msg.kind {
            let author = from?.id;
            let content = media_to_content(media_kind)?;
            let reply_to_message = reply_to_message?;
            let thread = reply_to_message.id;
            if let MessageKind::Common(MessageCommon{from, media_kind, ..}) = reply_to_message.kind {
                let replied_author = from?.id;
                let replied_content = media_to_content(media_kind)?;
                Some(Self{chat_id, url, thread, author, content, replied_author, replied_content})
            } else {
                log::error!("cannot invoke replied message: {:?}", reply_to_message.kind);
                None
            }
        } else {
            None
        }
    } 
}
