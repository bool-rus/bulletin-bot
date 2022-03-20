use std::sync::Arc;
use tbot::contexts::fields::{AnyText, Callback, Context, Message, Photo};
use tbot::errors;
use tbot::types::parameters::ChatId;
use tbot::contexts::DataCallback;

use super::fsm::*;

pub trait LoggableErrorResult<T> {
    fn ok_or_log(self) -> Option<T>;
}

impl<T,E: std::fmt::Debug> LoggableErrorResult<T> for Result<T,E> {
    fn ok_or_log(self) -> Option<T> {
        match self  {
            Ok(obj) => Some(obj),
            Err(e) => {
                log::error!("{:?}", e);
                None
            }
        }
    }
}

impl IncomeMessage for Arc<tbot::contexts::Text> {
    fn text(&self) -> Option<String> {
        Some(self.text.value.to_owned())
    }

    fn photo_id(&self) -> Option<String> {
        None
    }

    fn author(&self) -> Option<i64> {
        invoke_author(self)
    }
}

fn invoke_author<T: AnyText>(text: &Arc<T>) -> Option<i64> {
    let text = text.text();
    text.entities.iter().filter_map(|e| {
        if let tbot::types::message::text::EntityKind::TextMention(user) = &e.kind {
            Some(user)
        } else { None }
    }).last().map(|user| {
        user.id.0
    })
}

impl IncomeMessage for Arc<tbot::contexts::Photo> {
    fn text(&self) -> Option<String> {
        let text = self.caption.value.clone();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    fn photo_id(&self) -> Option<String> {
        self.photo().last().map(|size|size.file_id.0.clone())
    }

    fn author(&self) -> Option<i64> {
        invoke_author(self)
    }
}

pub async fn invoke_username(bot: &tbot::Bot, id: i64) -> String {
    bot.get_chat_member(ChatId::from(id), id.into()).call().await.map(|user| {
        let user = user.user;
        format!("{} {}", user.first_name, user.last_name.unwrap_or_default())
    }).unwrap_or(format!("Unknown({})", id))
}

async fn publish_ad<T: ContextEx>(ctx: &T, ad: &Ad, chat_id: crate::ChannelId) -> Result<Vec<tbot::types::Message>, errors::MethodCall> {
    use tbot::markup::*;
    use tbot::types::parameters::Text;
    use num_format::{ToFormattedString, Locale};
    let name = if let Some(user) = ctx.from_user() {
        let first = user.first_name.clone();
        user.last_name.as_ref().map(|second|{ 
            format!("{} {}", first, second)
        }).unwrap_or(first)
    } else {
        "анон".to_owned()
    };
    let price = ad.price.to_formatted_string(&Locale::ru);
    let text = markdown_v2((
        ad.text.as_str(),
        "\n\n", bold(price.as_str()), " ₽\n",
         "Прислано ", 
         mention(name.as_str(), ctx.chat_id().0.into())
    )).to_string();
    let content = Text::with_markdown_v2(text.as_str());
    if ad.photos.len() > 0 {
        let mut photos: Vec<_> = ad.photos.iter().map(String::as_str).map(|file_id|{
            tbot::types::input_file::Photo::id(file_id)
        }).collect();
        photos[0] = photos[0].caption(content);
        let photos: Vec<_> = photos.into_iter().map(Into::into).collect();
        ctx.bot().send_media_group(chat_id, photos.as_slice()).call().await
    } else {
        ctx.bot().send_message(chat_id, content).call().await.map(|m|vec![m])
    }
}

pub struct User {
    id: tbot::types::user::Id,
    first_name: String,
    last_name: Option<String>,
}

impl From<&tbot::types::User> for User {
    fn from(u: &tbot::types::User) -> Self {
        Self {
            id: u.id,
            first_name: u.first_name.clone(),
            last_name: u.last_name.clone(),
        }
    }
}

pub trait MyMessage: Message {}
impl MyMessage for tbot::contexts::Text {}
impl MyMessage for tbot::contexts::Photo {}
impl MyMessage for tbot::contexts::Command<tbot::contexts::Text> {}

pub trait ContextEx {
    fn from_user(&self) -> Option<User>;
    fn chat_id(&self) -> tbot::types::chat::Id;
    fn bot(&self) -> &tbot::Bot;
}

impl<T:MyMessage> ContextEx for T {
    fn from_user(&self) -> Option<User> {
        self.from().map(Into::into)
    }
    fn chat_id(&self) -> tbot::types::chat::Id {
        self.chat().id
    }
    fn bot(&self) -> &tbot::Bot {
        Context::bot(self)
    }
}

impl ContextEx for DataCallback {
    fn from_user(&self) -> Option<User> {
        Some(self.from().into())
    }
    fn chat_id(&self) -> tbot::types::chat::Id {
        let id = self.from.id.0;
        tbot::types::chat::Id(id)
    }
    fn bot(&self) -> &tbot::Bot {
        Context::bot(self)
    }
}

pub async fn do_response<T: ContextEx>(ctx: &T, response: Response, channel: crate::ChannelId) {
    let chat_id = ctx.chat_id();
    let bot = ctx.bot();
    match response {
        Response::Unban(id) => {
            let text = format!("Принято, разбанил {}", invoke_username(bot, id).await);
            bot.send_message(chat_id, text.as_str()).call().await.ok_or_log();
        }
        Response::BannedUsers(ids) => {
            use tbot::types::keyboard::inline::{Button, ButtonKind};
            let mut users = Vec::with_capacity(ids.len());
            for id in ids {
                let user = bot.get_chat_member(ChatId::from(id), id.into()).call().await
                .map(|user| {
                    let user = user.user;
                    format!("{} {}", user.first_name, user.last_name.unwrap_or_default())
                }).unwrap_or(format!("Unknown({})", id));
                users.push((user, ron::to_string(&CallbackResponse::User(id)).unwrap()));
            }
            let buttons_owner: Vec<_> = users.iter().map(|(name, id)|{
                vec![Button::new(name.as_str(), ButtonKind::CallbackData(id.as_str()))]
            }).collect();
            let buttons: Vec<_> = buttons_owner.iter().map(|x|x.as_slice()).collect();
            bot.send_message(chat_id, "Выбери, кого амнистировать:").reply_markup(buttons.as_slice()).call().await.ok_or_log();
        }
        Response::FirstCreate => { bot.send_message(chat_id, "Сначала надо создать объявление").call().await.ok_or_log(); } 
        Response::PriceRequest => { bot.send_message(chat_id, "Назови свою цену").call().await.ok_or_log(); }
        Response::NotPrice => { bot.send_message(chat_id, "Это не цена").call().await.ok_or_log(); }
        Response::FillRequest => { bot.send_message(chat_id, "Присылай описание или фотки").call().await.ok_or_log(); }
        Response::ContinueFilling => { bot.send_message(chat_id, "Теперь можешь заменить описание или добавить фото (не более 10)").call().await.ok_or_log(); }
        Response::WrongMessage => { bot.send_message(chat_id, "Что-то не то присылаешь").call().await.ok_or_log(); }
        Response::CannotPublish => { bot.send_message(chat_id, "Пока не могу опубликовать").call().await.ok_or_log(); }
        Response::Preview(ad) => if let Some(_) = publish_ad(ctx, &ad, chat_id).await.ok_or_log() {
            use tbot::types::keyboard::inline::{Button, ButtonKind};
            let yes = ron::to_string(&CallbackResponse::Yes).unwrap();
            let no = ron::to_string(&CallbackResponse::No).unwrap();
            let markup: &[&[Button]] = &[&[
                Button::new("Да", ButtonKind::CallbackData(yes.as_str())),
                Button::new("Нет", ButtonKind::CallbackData(no.as_str())),
            ]];
            bot.send_message(chat_id, "Все верно?").reply_markup(markup).call().await.ok_or_log();
        } else {
            bot.send_message(chat_id, "Не удалось подготовить объявление. Возможно, слишком много фото").call().await.ok_or_log();
        },
        Response::Publish(ad) => { 
            if let Some(msgs) = publish_ad(ctx, &ad, channel).await.ok_or_log() {
                use tbot::types::keyboard::inline::{Button, ButtonKind};
                let data = ron::to_string(&CallbackResponse::Remove(
                    msgs.iter().map(|msg|msg.id.0).collect()
                )).unwrap();
                let markup: &[&[Button]] = &[&[
                    Button::new("Снять с публикации", ButtonKind::CallbackData(data.as_str())),
                ]];
                bot.send_message(chat_id, "Объявление опубликовано:").reply_markup(markup).call().await.ok_or_log();
                bot.forward_message(chat_id, channel, msgs[0].id).call().await.ok_or_log();
            }
        }  
        Response::Ban(_, _) => { bot.send_message(chat_id, "Принято, больше не нахулиганит").call().await.ok_or_log(); }
        Response::Banned(cause) => { bot.send_message(chat_id, format!("Сорян, ты в бане.\nПричина: {}", cause).as_str()).call().await.ok_or_log(); }
        Response::ForwardMe => { bot.send_message(chat_id, "Пересылай объявление с нарушением").call().await.ok_or_log(); }
        Response::SendCause => { bot.send_message(chat_id, "Укажи причину бана").call().await.ok_or_log(); }
        Response::Remove(msgs) => {
            for id in msgs {
                bot.delete_message(channel, id.into()).call().await.ok_or_log();
            } //TODO: возможно, здесь нужно обработать исключения... или нет... посмотрим по логам.
            bot.send_message(chat_id, "Объявление удалено").call().await.ok_or_log();
        },
        Response::Empty => {  }
    }
}
