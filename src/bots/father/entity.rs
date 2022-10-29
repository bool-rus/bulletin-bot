use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CallbackResponse {
    Select(i64, String),
    Restart,
    Nothing,
    Remove(i64, String),
    EditTemplates,
    EditTemplate(usize),
    ResetTemplate,
}

//TODO: убрать копипасту с bulletin::entity
impl TryFrom<&str> for CallbackResponse {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let bytes = base91::slice_decode(value.as_bytes());
        postcard::from_bytes::<CallbackResponse>(bytes.as_slice()).map_err(|e|e.into())
    }
}

impl CallbackResponse {
    pub fn to_string(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let buf = postcard::to_vec::<_, 64>(&self)?;
        let encoded = base91::slice_encode(buf.as_slice());
        Ok(String::from_utf8(encoded)?)
    }
}