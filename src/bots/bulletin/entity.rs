use std::str::FromStr;
use super::*;

use serde::{Serialize, Deserialize};

use teloxide::{types::{UserId, Update, ChatId, UpdateKind, MessageKind, MessageCommon, MediaKind, PhotoSize, User}, dispatching::dialogue::GetChatId};

type MessageId = i32;

#[derive(Serialize, Deserialize, Clone)]
pub enum CallbackResponse {
    #[serde(rename="y")]
    Yes,
    #[serde(rename="n")]
    No,
    User(UserId),
    Remove(Vec<MessageId>),
}

#[derive(Clone)]
pub enum Content {
    Text(String),
    Photo(String),
    TextAndPhoto(String, String),
}

#[derive(Clone)]
pub enum Command {
    Help,
    Create,
    Publish,
    Ban,
    Unban,
}

#[derive(Clone)]
pub enum SignalKind {
    Command(Command),
    Content(Content),
    Select(CallbackResponse),
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
                            match photo.caption {
                                Some(text) => SignalKind::Content(Content::TextAndPhoto(text, best_size)),
                                None => SignalKind::Content(Content::Photo(best_size)),
                            }
                        },
                        MediaKind::Text(m) => {
                            let text = m.text.as_str();
                            match Command::from_str(text) {
                                Ok(cmd) => SignalKind::Command(cmd),
                                Err(_) => SignalKind::Content(Content::Text(m.text)),
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
                    Ok(response) => Some(Signal{chat_id, user, kind: SignalKind::Select(response)}),
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
    pub fn filter_command(self) -> Option<(User, Command)> {
        match self.kind {
            SignalKind::Command(cmd) => Some((self.user, cmd)),
            _ => None,
        }
    }
    pub fn filter_content(self) -> Option<(User, Content)> {
        match self.kind {
            SignalKind::Content(c) => Some((self.user,c)),
            _ => None,
        }
    }
    pub fn filter_callback(self) -> Option<(User, CallbackResponse)> {
        match self.kind {
            SignalKind::Select(c) => Some((self.user,c)),
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
            "/help" => Ok(Self::Help),
            "/create" => Ok(Self::Create),
            "/publish" => Ok(Self::Publish),
            "/ban" => Ok(Self::Ban),
            "/unban" => Ok(Self::Unban),
            _ => Err(())
        }
    }
}

impl Content {
    pub fn price(&self) -> Option<Price> {
        match self {
            Self::Text(txt) => {
                txt.parse().ok()
            },
            _ => None
        }
    }
}


