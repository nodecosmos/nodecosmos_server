use crate::errors::NodecosmosError;
use crate::models::node::{GetDescriptionBase64Node, UpdateDescriptionNode};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use scylla::CachingSession;
use std::borrow::Cow;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, Transact, Update};

enum ProseMirrorXmlTag {
    Heading,
    Paragraph,
    Bold,
    Italic,
    Strike,
    Code,
    BulletList,
    OrderedList,
    ListItem,
    Blockquote,
    CodeBlock,
    Image,
    Link,
    HardBreak,
    Html,
}

impl<'a> From<QName<'a>> for ProseMirrorXmlTag {
    // I extracted this manually from the ProseMirror schema. It seems that these are tags
    // used by Remirror's implementation of ProseMirror. However, in order to be safe in the
    // long-term, we need our own implementation of ProseMirror.
    fn from(value: QName<'a>) -> Self {
        match value {
            QName(b"heading") => ProseMirrorXmlTag::Heading,
            QName(b"paragraph") => ProseMirrorXmlTag::Paragraph,
            QName(b"bold") => ProseMirrorXmlTag::Bold,
            QName(b"italic") => ProseMirrorXmlTag::Italic,
            QName(b"strike") => ProseMirrorXmlTag::Strike,
            QName(b"code") => ProseMirrorXmlTag::Code,
            QName(b"bulletList") => ProseMirrorXmlTag::BulletList,
            QName(b"orderedList") => ProseMirrorXmlTag::OrderedList,
            QName(b"listItem") => ProseMirrorXmlTag::ListItem,
            QName(b"blockquote") => ProseMirrorXmlTag::Blockquote,
            QName(b"codeBlock") => ProseMirrorXmlTag::CodeBlock,
            QName(b"image") => ProseMirrorXmlTag::Image,
            QName(b"link") => ProseMirrorXmlTag::Link,
            QName(b"hardBreak") => ProseMirrorXmlTag::HardBreak,
            QName(b"html") => ProseMirrorXmlTag::Html,
            _ => panic!("Unknown tag {:?}", value),
        }
    }
}

struct Description {
    html: String,
    markdown: String,
    short_description: String,
}

impl Description {
    const SHORT_DESCRIPTION_LENGTH: usize = 255;
    const ELLIPSIS: &'static str = "...";

    pub fn from_xml(xml: &str) -> Result<Self, NodecosmosError> {
        let mut reader = Reader::from_str(xml);
        let mut html = String::new();
        let mut markdown = String::new();
        let mut short_description = String::new();
        let mut heading_level = String::new();
        let mut current_href = String::new();
        let mut ordered_list_counter = 1;
        let mut paragraph_active = false;
        let mut ordered_list_active = false;
        let mut bullet_list_active = false;
        let mut blockquote_active = false;
        let mut current_text = Cow::Borrowed("");

        reader.trim_text(true);

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => match ProseMirrorXmlTag::from(e.name()) {
                    ProseMirrorXmlTag::Heading => {
                        heading_level = e
                            .attributes()
                            .find_map(|a| {
                                a.ok().filter(|a| a.key == QName(b"level")).map(|a| {
                                    a.decode_and_unescape_value(&reader)
                                        .expect("Error decoding heading level")
                                        .to_string()
                                })
                            })
                            .unwrap_or_else(|| "1".to_string());
                        html.push_str(&format!("<h{}>", heading_level));
                        let level = heading_level.parse::<u8>().expect("Error parsing heading level");

                        for _ in 0..level {
                            markdown.push('#');
                        }
                        markdown.push(' ');
                    }
                    ProseMirrorXmlTag::Paragraph => {
                        html.push_str("<p>");
                        paragraph_active = true;
                    }
                    ProseMirrorXmlTag::Bold => {
                        html.push_str("<strong>");
                        markdown.push_str("**");
                    }
                    ProseMirrorXmlTag::Italic => {
                        html.push_str("<em>");
                        markdown.push_str("_");
                    }
                    ProseMirrorXmlTag::Strike => {
                        html.push_str("<s>");
                        markdown.push_str("~~");
                    }
                    ProseMirrorXmlTag::Code => {
                        html.push_str("<code>");
                        markdown.push_str("`");
                    }
                    ProseMirrorXmlTag::BulletList => {
                        html.push_str("<ul>");
                        bullet_list_active = true;
                    }
                    ProseMirrorXmlTag::OrderedList => {
                        html.push_str("<ol>");
                        ordered_list_active = true;
                    }
                    ProseMirrorXmlTag::ListItem => {
                        html.push_str("<li>");

                        if ordered_list_active {
                            markdown.push_str(&format!("{}. ", ordered_list_counter));
                            ordered_list_counter += 1;
                        } else {
                            markdown.push_str("* ");
                        }
                    }
                    ProseMirrorXmlTag::Blockquote => {
                        html.push_str("<blockquote>");
                        blockquote_active = true;
                    }
                    ProseMirrorXmlTag::CodeBlock => {
                        html.push_str("<pre>");
                        markdown.push_str("```markup\n");
                    }
                    ProseMirrorXmlTag::Image => {
                        let src = e
                            .attributes()
                            .find_map(|a| {
                                a.ok()
                                    .filter(|a| a.key == QName(b"src"))
                                    .map(|a| a.decode_and_unescape_value(&reader).unwrap())
                            })
                            .unwrap_or_default();
                        let alt = e
                            .attributes()
                            .find_map(|a| {
                                a.ok()
                                    .filter(|a| a.key == QName(b"alt"))
                                    .map(|a| a.decode_and_unescape_value(&reader).unwrap())
                            })
                            .unwrap_or_default();
                        html.push_str(&format!("<img src=\"{}\" alt=\"{}\"/>", src, alt));
                        markdown.push_str(&format!("\n![{}]({})", alt, src));
                    }
                    ProseMirrorXmlTag::Link => {
                        let href = e
                            .attributes()
                            .find_map(|a| {
                                a.ok()
                                    .filter(|a| a.key == QName(b"href"))
                                    .map(|a| a.decode_and_unescape_value(&reader).unwrap())
                            })
                            .unwrap_or_default();
                        html.push_str(&format!("<a href=\"{}\" target = _blank>", href));
                        markdown.push_str("[");
                        current_href = href.to_string();
                    }
                    ProseMirrorXmlTag::HardBreak => {
                        println!("HardBreak");
                        html.push_str("<br/>");
                        markdown.push_str("\n");
                    }
                    ProseMirrorXmlTag::Html => (),
                },
                Ok(Event::Text(e)) => {
                    current_text = e.unescape()?;

                    html.push_str(&current_text);

                    if blockquote_active {
                        markdown.push_str(&format!("> {}", current_text));
                    } else {
                        markdown.push_str(&current_text);
                    }

                    if paragraph_active && short_description.is_empty() {
                        short_description = current_text.to_string();
                        short_description.truncate(Self::SHORT_DESCRIPTION_LENGTH - Self::ELLIPSIS.len());
                        short_description = short_description.trim_end().to_string();

                        if short_description.len() < current_text.len() {
                            short_description.push_str(Self::ELLIPSIS);
                        }
                    }
                }
                Ok(Event::End(ref e)) => match ProseMirrorXmlTag::from(e.name()) {
                    ProseMirrorXmlTag::Heading => {
                        html.push_str(&format!("</h{}>", heading_level));
                        markdown.push_str("\n\n");
                    }
                    ProseMirrorXmlTag::Paragraph => {
                        if ordered_list_active || bullet_list_active {
                            html.push_str("</p>");
                            markdown.push_str("\n");
                        } else if blockquote_active {
                            html.push_str("</p>");
                            markdown.push_str("\n>\n");
                        } else if current_text.len() > 0 {
                            html.push_str("</p>");
                            markdown.push_str("\n\n");
                        }

                        current_text = Cow::Borrowed("");
                        paragraph_active = false;
                    }
                    ProseMirrorXmlTag::Bold => {
                        html.push_str("</strong>");
                        markdown.push_str("** ");
                    }
                    ProseMirrorXmlTag::Italic => {
                        html.push_str("</em>");
                        markdown.push_str("_ ");
                    }
                    ProseMirrorXmlTag::Strike => {
                        html.push_str("</s>");
                        markdown.push_str("~~ ");
                    }
                    ProseMirrorXmlTag::Code => {
                        html.push_str("</code>");
                        markdown.push_str("` ");
                    }
                    ProseMirrorXmlTag::ListItem => {
                        html.push_str("</li>");
                        markdown.push_str("\n");
                    }
                    ProseMirrorXmlTag::BulletList => {
                        html.push_str("</ul>");
                        markdown.push_str("\n");
                        bullet_list_active = false;
                    }
                    ProseMirrorXmlTag::OrderedList => {
                        html.push_str("</ol>");
                        ordered_list_active = false;
                        ordered_list_counter = 1;
                        markdown.push_str("\n");
                    }
                    ProseMirrorXmlTag::Blockquote => {
                        html.push_str("</blockquote>");
                        blockquote_active = false;
                        markdown.push_str("\n");
                    }
                    ProseMirrorXmlTag::CodeBlock => {
                        html.push_str("</pre>");
                        markdown.push_str("\n```");
                        markdown.push_str("\n\n");
                    }
                    ProseMirrorXmlTag::Image => {
                        markdown.push_str("\n\n");
                    }
                    ProseMirrorXmlTag::Link => {
                        html.push_str("</a>");
                        markdown.push_str(&format!("]({})", current_href));
                        current_href = String::new();
                    }
                    ProseMirrorXmlTag::HardBreak => (),
                    ProseMirrorXmlTag::Html => (),
                },
                Ok(Event::Eof) => break,
                _ => (),
            }
        }

        Ok(Self {
            html,
            markdown: markdown.trim_end().to_string(),
            short_description,
        })
    }
}

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

        let prose_doc = Description::from_xml(&xml.get_string(&transaction))?;
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
