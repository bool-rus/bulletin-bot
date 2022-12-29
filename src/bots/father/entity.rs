use serde::{Serialize, Deserialize};

use crate::bots::CallbackMessage;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CallbackResponse {
    Select(i64, String),
    Restart,
    Nothing,
    Remove(i64, String),
    EditTemplates,
    EditTemplate(usize),
    ResetTemplate,
    UpdateToken,
    AddTag,
    RemoveTag,
    TagToRemove(String),
}

impl CallbackMessage for CallbackResponse {}