use super::fsm::*;
use telegram_bot::{Message, MessageKind, MessageEntityKind, MessageEntity};

impl IncomeMessage for Message {
    fn text(&self) -> Option<String> {
        match &self.kind {
            MessageKind::Text { data, entities } => Some(data.clone()),
            MessageKind::Photo { data, caption, media_group_id } => caption.clone(),
            _ => None
        }
    }

    fn photo_id(&self) -> Option<String> {
        match &self.kind {
            MessageKind::Photo { data, caption, media_group_id } => {
                data.last().map(|ps|ps.file_id.clone())
            }
            _ => None
        }
    }

    fn author(&self) -> Option<i64> {
        todo!()
    }
}

impl From<Message> for Signal<Message> {
    fn from(m: Message) -> Self {

        match &m.kind {
            MessageKind::Text { data, entities } => {
                for entity in entities {
                    if matches!(entity.kind, MessageEntityKind::BotCommand) {
                        match invoke_entity(entity, data).as_str() {
                            "/create" => return Self::Create,
                            "/publish" => return Self::Publish,
                            "/ban" => return Self::Ban,
                            _ => return Self::Unknown,
                        }
                    }
                }
                Signal::Message(m)
            }
            _ => Signal::Message(m)
        }
    }
}


fn invoke_entity(entity: &MessageEntity, data: &str) -> String {
    let MessageEntity {offset, length, ..} = entity;
    let chars: Vec<_> = data.encode_utf16().skip(*offset as usize).take(*length as usize).collect();
    String::from_utf16(&chars).unwrap()
} 