use serde::{Serialize, Deserialize};

use crate::bots::CallbackMessage;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CallbackResponse {
    Select(i64),
    Restart,
    Nothing,
    Remove(i64),
    EditTemplates,
    EditTemplate(usize),
    ResetTemplate,
    UpdateToken,
    AddTag,
    RemoveTag,
    TagToRemove(String),
    Options,
    ToggleOption(i32),
    Back,
    Save,
}

impl CallbackMessage for CallbackResponse {}