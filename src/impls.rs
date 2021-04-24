use std::sync::Arc;

use tbot::{contexts::{Photo, Text, fields::{Caption, MediaMessage, Photo as _}, methods::ChatMethods}, types::{file::id::AsFileId, input_file::GroupMedia, }};
use tbot::markup::markdown_v2;
use super::fsm::*;

struct File {
    caption: String, 
    id: tbot::types::file::Id,
} 

pub struct Ad {
    pub price: u32,
    pub text: String, 
    media: Vec<File>,
}

impl From<u32> for Ad {
    fn from(price: u32) -> Self {
        Self { price, text: String::new(), media: Vec::new(), }
    }
}

impl AsPrice<u32> for Arc<Text> {
    fn as_price(&self) -> Option<u32> {
        self.text.value.parse::<u32>().ok()
    }
}

impl AsPrice<u32> for Arc<Photo> {
    fn as_price(&self) -> Option<u32> {
        None
    }
}

impl AsPrice<u32> for () {
    fn as_price(&self) -> Option<u32> {
        None
    }
}

impl FillableFrom<()> for Ad {
    fn fill_from(&mut self, _: ()) {}
}

impl FillableFrom<Arc<Text>> for Ad {
    fn fill_from(&mut self, item: Arc<Text>) {
        self.text = item.text.value.to_owned();
    }
}

impl FillableFrom<Arc<Photo>> for Ad {
    fn fill_from(&mut self, item: Arc<Photo>) {
        if let Some(file) = item.photo().last() {
            self.media.push(File{
                caption: item.caption().value.to_owned(),
                id: file.as_file_id().to_owned(),
            })
        }
    }
}

impl Ad {
    fn text(&self) -> String {
        format!("{}\n\nЦена: {} р", self.text, self.price)
    }
    fn album(&self) -> Vec<tbot::types::input_file::Photo> {
        self.media.iter().map(|file|{
            tbot::types::input_file::Photo::with_id(file.id.as_file_id())
        }).collect()
    }
}

pub async fn send_response<C: ChatMethods>(channel: tbot::types::chat::Id, ctx: &C, response: Response<Ad>) {
    match response {
        Response::FirstCreate => {ctx.send_message("Сначала скомандуй /create").call().await;},
        Response::PriceRequest => {ctx.send_message( "Назови свою цену").call().await;},
        Response::NotPrice => {ctx.send_message("Это не цена").call().await;},
        Response::FillRequest => {ctx.send_message("Присылай описание или фотки").call().await;},
        Response::ContinueFilling => {ctx.send_message("Что-то еще?").call().await;},
        Response::WrongMessage => {ctx.send_message("Что-то не то присылаешь").call().await;},
        Response::CannotPublish => {ctx.send_message("Пока не могу опубликовать").call().await;},
        Response::Publish(ad) => {
            let mut photos = ad.album();
            let content = if let Some(from) = ctx.from().clone() {
                let from = from.clone();
                format!("{}\nПрислано @{}", ad.text(), from.username.unwrap_or("anonymous".to_owned()))
            } else {
                ad.text()
            };
            let text = content.as_str();//tbot::types::parameters::Text::with_markdown_v2(content.as_str());
            if photos.is_empty() {
                if let Err(e) = ctx.bot().send_message(channel, text).call().await {
                    println!("Some trouble {:?}", e);
                }
            } else {
                photos[0] = photos[0].caption(text);
                let photos: Vec<_> = photos.into_iter().map(|p|GroupMedia::Photo(p)).collect();
                ctx.bot().send_media_group(channel, photos.as_slice()).call().await;
            }
        }
    }
}
