use crate::errors::NodecosmosError;
use crate::models::node::{GetDescriptionBase64Node, UpdateDescriptionNode};
use crate::models::utils::DescriptionXmlParser;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use scylla::CachingSession;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, Transact, Update};

pub trait MergeDescription {
    const DESCRIPTION_ROOT: &'static str = "prosemirror";

    async fn current_base64(&self, db_session: &CachingSession) -> Result<String, NodecosmosError>;
    async fn updated_base64(&self) -> Result<String, NodecosmosError>;

    fn assign_html(&mut self, html: String);
    fn assign_markdown(&mut self, markdown: String);
    fn assign_short_description(&mut self, short_description: String);
    fn assign_base64(&mut self, short_description: String);

    async fn merge_description(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let current_base64 = self.current_base64(db_session).await?;
        let updated_base64 = self.updated_base64().await?;
        let current_buf = STANDARD.decode(current_base64)?;
        let update_buf = STANDARD.decode(updated_base64)?;
        let current = Update::decode_v2(&current_buf)?;
        let update = Update::decode_v2(&update_buf)?;
        let doc = Doc::new();
        let xml = doc.get_or_insert_xml_fragment(Self::DESCRIPTION_ROOT);
        let mut transaction = doc.transact_mut();

        transaction.apply_update(current);
        transaction.apply_update(update);

        let xml_str = &xml.get_string(&transaction);
        let prose_doc = DescriptionXmlParser::new(xml_str).run()?;
        let html = prose_doc.html;
        let markdown = prose_doc.markdown;
        let short_description = prose_doc.short_description;
        let base64 = STANDARD.encode(&transaction.encode_update_v2());

        self.assign_html(html);
        self.assign_markdown(markdown);
        self.assign_short_description(short_description);
        self.assign_base64(base64);

        Ok(())
    }

    async fn undo_merge_description(&mut self, db_session: &CachingSession) -> Result<(), NodecosmosError> {
        let current_base64 = self.current_base64(db_session).await?;
        let current_buf = STANDARD.decode(current_base64)?;
        let current = Update::decode_v2(&current_buf)?;
        let doc = Doc::new();
        let xml = doc.get_or_insert_xml_fragment(Self::DESCRIPTION_ROOT);
        let mut transaction = doc.transact_mut();

        transaction.apply_update(current);

        let xml_str = &xml.get_string(&transaction);
        let prose_doc = DescriptionXmlParser::new(xml_str).run()?;
        let html = prose_doc.html;
        let markdown = prose_doc.markdown;
        let short_description = prose_doc.short_description;
        let base64 = STANDARD.encode(&transaction.encode_update_v2());

        self.assign_html(html);
        self.assign_markdown(markdown);
        self.assign_short_description(short_description);
        self.assign_base64(base64);

        Ok(())
    }
}

/// In future it can be used for description of other models like IO, Flow, Flow Step, etc.
impl MergeDescription for UpdateDescriptionNode {
    async fn current_base64(&self, db_session: &CachingSession) -> Result<String, NodecosmosError> {
        let node =
            GetDescriptionBase64Node::find_branched_or_original(db_session, self.id, self.branch_id, None).await?;

        Ok(node.description_base64.unwrap_or(
            self.description_base64
                .clone()
                .expect("Description base64 is required."),
        ))
    }

    async fn updated_base64(&self) -> Result<String, NodecosmosError> {
        Ok(self.description_base64.clone().unwrap_or_default())
    }

    fn assign_html(&mut self, html: String) {
        self.description = Some(html);
    }

    fn assign_markdown(&mut self, markdown: String) {
        self.description_markdown = Some(markdown);
    }

    fn assign_short_description(&mut self, short_description: String) {
        self.short_description = Some(short_description);
    }

    fn assign_base64(&mut self, base64: String) {
        self.description_base64 = Some(base64);
    }
}
