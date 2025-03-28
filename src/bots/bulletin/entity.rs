use super::*;
use super::res::*;

use serde::{Serialize, Deserialize};

use teloxide::dispatching::dialogue::GetChatId;
use teloxide::types::{UserId, Update, ChatId, UpdateKind, MessageKind, MediaKind, MediaText, MessageCommon, MessageId};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CallbackResponse {
    #[serde(rename="y")]
    Yes,
    #[serde(rename="n")]
    No,
    Target(Target),
    User(UserId),
    Remove(Vec<i32>),
    AdminToRemove(UserId),
    AddTag(String, i32),
    RemoveTag(String, i32),
    ApproveSubscribe(UserId, ChatId),
    DeclineSubscribe(UserId),
    BanSubscribe(UserId),
}

impl CallbackMessage for CallbackResponse {}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Buy,
    Sell,
    Ask,
    Recommend,
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
    Remove(Vec<i32>),
    AddTag(String, i32),
    RemoveTag(String, i32),
}

#[derive(Clone, Debug)]
pub enum AdminAction {
    Ban,
    Unban,
    UserToUnban(UserId),
    AddAdmin,
    RemoveAdmin,
    AdminToRemove(UserId),
    ApproveSubscribe(UserId, ChatId),
    DeclineSubscribe(UserId),
    BanSubscribe(UserId),
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
    pub fn from_update(u: Update, conf: Conf) -> Option<Self> {
        match u.kind {
            UpdateKind::Message(msg) | UpdateKind::EditedMessage(msg) => {
                let chat_id = msg.chat.id;
                let kind = if let MessageKind::Common(msg) = msg.kind {
                    let content = media_to_content(msg.media_kind)?;
                    if let Some(cmd) = content.command(conf.clone()) {
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
                match CallbackResponse::from_mst_text(data.as_str()) {
                    Ok(response) => Some(Signal{chat_id, kind: response.into()}),
                    Err(e) => {
                        log::error!("cannot parse callback data: {} error: {:?}", data, e);
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

impl Command {
    fn from_str(s: &str, _conf: Conf) -> Option<Self> {
        Some(match s {
            "/help" | "/start" => Self::Help,
            "/create" | CREATE => Self::Create,
            "/publish" | PUBLISH => Self::Publish,
            "/ban" | BAN => Self::Ban,
            "/unban" | UNBAN => Self::Unban,
            ADD_ADMIN => Self::AddAdmin,
            REMOVE_ADMIN => Self::RemoveAdmin,
            _ => return None
        })
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
        use CallbackResponse::*;
        use UserAction as U;
        use AdminAction as A;
        match self {
            Yes => SK::UserAction(U::Yes),
            No => SK::UserAction(U::No),
            User(u) => SK::AdminAction(A::UserToUnban(u)),
            AdminToRemove(u) => SK::AdminAction(A::AdminToRemove(u)),
            Remove(msgs) => SK::UserAction(U::Remove(msgs)),
            Target(target) => SK::UserAction(U::Target(target)),
            AddTag(tag, msg_id) => SK::UserAction(U::AddTag(tag, msg_id)),
            RemoveTag(tag, msg_id) => SK::UserAction(U::RemoveTag(tag, msg_id)),
            ApproveSubscribe(id,chat_id) => SK::AdminAction(A::ApproveSubscribe(id, chat_id)),
            DeclineSubscribe(id) => SK::AdminAction(A::DeclineSubscribe(id)),
            BanSubscribe(id) => SK::AdminAction(A::BanSubscribe(id)),
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
    fn command(&self, conf: Conf) -> Option<Command> {
        match self {
            Self::Text(txt) => {
                Command::from_str(txt.text.as_str(), conf)
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
            let best_size = photo.photo.last()?.file.id.clone();
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
    pub id: MessageId,
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
    Ban(UserId),
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
                let thread = reply_to_message.id.0;
                let content = media_to_content(media_kind)?;
                if let MessageKind::Common(MessageCommon{from, media_kind, ..}) = reply_to_message.kind {
                    let replied_author = from?.id;
                    if replied_author.is_telegram() { 
                        let replied_content = media_to_content(media_kind)?;
                        let replied_author = invoke_author(&replied_content)?;
                        //TODO: надо что-то придумать с дублированием
                        if content.text()?.to_lowercase() == conf.template(config::Template::BanCommand).to_lowercase(){
                            GroupMessageKind::Ban(replied_author)
                        } else {
                            GroupMessageKind::Comment { thread, replied_author}
                        }
                    } else if content.text()?.to_lowercase() == conf.template(config::Template::MuteCommand).to_lowercase() {
                        GroupMessageKind::Mute(replied_author)
                    } else if content.text()?.to_lowercase() == conf.template(config::Template::BanCommand).to_lowercase(){
                        GroupMessageKind::Ban(replied_author)
                    } else {
                        GroupMessageKind::Dumb
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
