use std::sync::Arc;
use log::warn;
use tbot::{contexts::{fields::{AnyText, Message, Photo}, methods::ChatMethods}, types::input_file::GroupMedia};

use super::fsm::*;

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

pub async fn do_response<T:Message>(api: Arc<T>, response: Response, channel: crate::ChannelId) {
    match response {
        Response::FirstCreate => { api.send_message("Сначала скомандуй /create").call().await; } 
        Response::PriceRequest => { api.send_message("Назови свою цену").call().await; }
        Response::NotPrice => { api.send_message("Это не цена").call().await; }
        Response::FillRequest => { api.send_message("Присылай описание или фотки").call().await; }
        Response::ContinueFilling => { api.send_message("Что-то еще?").call().await; }
        Response::WrongMessage => { api.send_message("Что-то не то присылаешь").call().await; }
        Response::CannotPublish => { api.send_message("Пока не могу опубликовать").call().await; }
        Response::Publish(ad) => { 
            use tbot::markup::*;
            use tbot::types::parameters::Text;
            let name = if let Some(user) = api.from() {
                let first = user.first_name.clone();
                user.last_name.as_ref().map(|second|{
                    format!("{} {}", first, second)
                }).unwrap_or(first)
            } else {
                "анон".to_owned()
            };
            let price = ad.price.to_string();
            let text = markdown_v2((
                ad.text.as_str(),
                "\nЦена ", bold(price.as_str()), " ₽\n",
                 "Прислано ", 
                 mention(name.as_str(), api.chat().id.0.into())
            )).to_string();
            let content = Text::with_markdown_v2(text.as_str());
            if ad.photos.len() > 0 {
                let mut photos: Vec<_> = ad.photos.iter().map(String::as_str).map(|file_id|{
                    tbot::types::input_file::Photo::id(file_id)
                }).collect();
                photos[0] = photos[0].caption(content);
                let photos: Vec<_> = photos.into_iter().map(Into::into).collect();
                warn!("photos: {:?}", photos);
                api.bot().send_media_group(channel, photos.as_slice()).call().await;
            } else {
                api.bot().send_message(channel, content).call().await;
            }

        }  
        Response::Ban(user_id, cause) => { api.send_message("Принято, больше не нахулиганит").call().await; }
        Response::Banned(cause) => { api.send_message(format!("Сорян, ты в бане.\nПричина: {}", cause).as_str()).call().await; }
        Response::ForwardMe => { api.send_message("Пересылай объявление с нарушением").call().await; }
        Response::SendCause => { api.send_message("Укажи причину бана").call().await; }
        Response::Empty => {  }
    }
}


/*
fn invoke_entity(entity: &MessageEntity, data: &str) -> String {
    let MessageEntity {offset, length, ..} = entity;
    let chars: Vec<_> = data.encode_utf16().skip(*offset as usize).take(*length as usize).collect();
    String::from_utf16(&chars).unwrap()
} 
*/