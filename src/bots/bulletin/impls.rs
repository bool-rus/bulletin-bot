use super::{*, fsm::FSMResult};
use teloxide::types::{UserId, Update, ChatId, UpdateKind, MessageKind, MessageCommon, MediaKind, User};
use teloxide::utils::markdown::*;


pub fn make_ad_text(user: User, ad: &Ad) -> String {
    let text = escape(&ad.text);
    let price = bold(&format!("{} ₽", ad.price));
    let sign = user_mention(user.id.0.try_into().unwrap(), &user.full_name());
    format!("{}\n\n{}\n{}\n", text, price, sign)
}
