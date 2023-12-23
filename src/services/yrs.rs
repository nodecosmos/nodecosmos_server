use crate::errors::NodecosmosError;
use crate::services::prosemirror::ProseMirrorDoc;
use base64::decode;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, Transact, TransactionMut, Update, XmlElementRef};

pub struct YrsDescription {
    pub current: Vec<u8>,
    pub update: Vec<u8>,
    pub doc: Doc,
    pub xml: XmlElementRef,
    pub html: String,
    pub markdown: String,
}

impl YrsDescription {
    pub fn new(current: String, update: String) -> Self {
        let current = decode(current).unwrap();
        let update = decode(update).unwrap();
        let mut doc = Doc::new();
        let xml = doc.get_or_insert_xml_element("prosemirror");

        Self {
            current,
            update,
            doc,
            xml,
            html: String::new(),
            markdown: String::new(),
        }
    }

    pub fn merge_description(&mut self) -> Result<(), NodecosmosError> {
        let mut transaction = self.doc.transact_mut();

        let current = Update::decode_v2(&self.current).unwrap();
        transaction.apply_update(current);

        // merge description update
        let update = Update::decode_v2(&self.update).unwrap();
        transaction.apply_update(update);

        let prose_doc = ProseMirrorDoc::from_xml(&self.xml.get_string(&transaction));
        self.html = prose_doc.html;
        self.markdown = prose_doc.markdown;

        Ok(())
    }
}
