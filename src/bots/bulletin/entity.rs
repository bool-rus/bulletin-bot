use std::str::FromStr;
use super::*;

use serde::{Serialize, Deserialize};

use teloxide::{types::{UserId, Update, ChatId, UpdateKind, MessageKind, MessageCommon, MediaKind, PhotoSize, User, MediaText}, dispatching::dialogue::GetChatId};

type MessageId = i32;

pub type Text = MediaText;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CallbackResponse {
    #[serde(rename="y")]
    Yes,
    #[serde(rename="n")]
    No,
    User(UserId),
    Remove(Vec<MessageId>),
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
            UpdateKind::Message(msg) => {
                let chat_id = msg.chat.id;
                let kind = if let MessageKind::Common(msg) = msg.kind {
                    user = msg.from?;
                    match msg.media_kind {
                        MediaKind::Photo(mut photo) => {
                            photo.photo.sort_unstable_by_key(|size|size.height);
                            let best_size = photo.photo.last()?.file_id.clone();
                            let entities = photo.caption_entities;
                            match photo.caption {
                                Some(text) => SignalKind::Content(Content::TextAndPhoto(MediaText { text, entities }, best_size)),
                                None => SignalKind::Content(Content::Photo(best_size)),
                            }
                        },
                        MediaKind::Text(m) => {
                            let text = m.text.as_str();
                            match Command::from_str(text) {
                                Ok(cmd) => cmd.into(),
                                Err(_) => SignalKind::Content(Content::Text(m)),
                            }
                        },
                        _ => return None
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
    //тут кортеж (User,Command) - это что-то уродливое. Тут либо делать типы-обертки, а-ля CommandSignal{user,command}, либо еще подумать
    pub fn filter_user_action(self) -> Option<(User, UserAction)> {
        match self.kind {
            SignalKind::UserAction(action) => Some((self.user, action)),
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
            "/help" => Ok(Self::Help),
            "/create" => Ok(Self::Create),
            "/publish" => Ok(Self::Publish),
            "/ban" => Ok(Self::Ban),
            "/unban" => Ok(Self::Unban),
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
}
