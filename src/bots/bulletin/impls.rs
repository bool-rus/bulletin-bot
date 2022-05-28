use super::*;
use teloxide::types::{ChatId, User, InputFile, ParseMode, InputMedia, InputMediaPhoto, UserId};
use teloxide::utils::markdown::*;


fn make_ad_text(user: &User, ad: &Ad) -> String {
    let text = escape(&ad.text);
    let price = bold(&format!("{} ₽", ad.price));
    let full_name = escape(&user.full_name());
    let sign = user_mention(user.id.0.try_into().unwrap(), &full_name);
    format!("{}\n\n{}\n{}\n", text, price, sign)
}

pub async fn send_ad(bot: WBot, conf: fsm::Conf, target_chat_id: ChatId, user_id: UserId, ad: &Ad) -> Result<Vec<Message>, Box<dyn std::error::Error + Send + Sync>> {

    let chat_member = bot.get_chat_member(conf.channel, user_id).await?;
    if chat_member.is_left() || chat_member.is_banned() {
        bot.send_message(target_chat_id, "Ты не с нами. Уходи").await?;
        Err("Пользователь не подписан на канал")?
    };
    let user = chat_member.user;
    let text = make_ad_text(&user, ad);
    let bot = bot.parse_mode(ParseMode::MarkdownV2);
    let mut photos: Vec<_> = ad.photos.iter().map(make_photo).collect();
    let msgs = if photos.is_empty() {
        vec![bot.send_message(target_chat_id, text).await?]
    } else {
        photos.first_mut().map(|photo|{
            photo.caption = Some(text);
            photo.parse_mode = Some(ParseMode::MarkdownV2);
        });
        let media: Vec<_> = photos.into_iter().map(|p|InputMedia::Photo(p)).collect();
        bot.send_media_group(target_chat_id, media).await?
    };
    Ok(msgs)
}

fn make_photo<T: Into<String>>(file_id: T) -> InputMediaPhoto {
   InputMediaPhoto::new(InputFile::file_id(file_id.into()))
}

