use std::collections::HashSet;

use super::entity::{Content, Target};
use super::Price;

#[derive(Debug, Clone)]
pub struct Ad {
    pub target: Target,
    pub price: Price,
    pub text: String,
    pub photos: Vec<String>,
    pub tags: HashSet<String>,
}
impl Ad {
    pub fn new(target: Target, price: Price) -> Self {
        Self {
            target,
            price,
            text: String::new(),
            photos: Vec::new(),
            tags: HashSet::new(),
        }
    }
    pub fn fill(&mut self, content: Content) {
        match content {
            Content::Text(text) => self.text = text.text,
            Content::Photo(id) => self.photos.push(id),
            Content::TextAndPhoto(text, id) => {
                self.text = text.text;
                self.photos.push(id);
            },
        }
    }
}