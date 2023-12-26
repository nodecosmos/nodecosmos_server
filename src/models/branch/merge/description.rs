use crate::errors::NodecosmosError;
use crate::services::prosemirror::ProseMirrorDoc;
use base64::engine::general_purpose::STANDARD;
use base64::{decode, Engine};
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, Transact, TransactionMut, Update, XmlElementRef};

pub struct DescriptionMerge {
    pub html: String,
    pub markdown: String,
    pub short_description: String,
    pub base64: String,
}

impl DescriptionMerge {
    pub const DESCRIPTION_ROOT: &'static str = "prosemirror";

    pub fn new(current: String, update: String) -> Self {
        let current_buf = STANDARD.decode(current).unwrap();
        let update_buf = STANDARD.decode(update).unwrap();
        let current = Update::decode_v2(&current_buf).unwrap();
        let update = Update::decode_v2(&update_buf).unwrap();
        let mut doc = Doc::new();
        let xml = doc.get_or_insert_xml_fragment(Self::DESCRIPTION_ROOT);
        let mut transaction = doc.transact_mut();

        transaction.apply_update(current);
        transaction.apply_update(update);

        let prose_doc = ProseMirrorDoc::from_xml(&xml.get_string(&transaction));
        let html = prose_doc.html;
        let markdown = prose_doc.markdown;
        let short_description = prose_doc.short_description;
        let base64 = STANDARD.encode(&transaction.encode_update_v2());

        Self {
            html,
            markdown,
            short_description,
            base64,
        }
    }
}
