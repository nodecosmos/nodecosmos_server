use charybdis::macros::charybdis_udt_model;
use charybdis::types::Text;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[charybdis_udt_model(type_name = TextChange)]
pub struct TextChange {
    pub old: Text,
    pub new: Text,
}

impl TextChange {
    pub fn new() -> Self {
        TextChange {
            old: Text::new(),
            new: Text::new(),
        }
    }

    pub fn assign_old(&mut self, old: Option<Text>) {
        self.old = old.unwrap_or_default();
    }

    pub fn assign_new(&mut self, new: Option<Text>) {
        self.new = new.unwrap_or_default();
    }
}
