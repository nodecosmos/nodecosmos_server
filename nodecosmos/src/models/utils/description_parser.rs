use crate::errors::NodecosmosError;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;
use yrs::{GetString, TransactionMut, Xml, XmlFragment, XmlFragmentRef, XmlOut};

pub trait DescriptionParser<'a> {
    const SHORT_DESCRIPTION_LENGTH: usize = 255;
    const ELLIPSIS: &'static str = "...";

    fn push_html(&mut self, tag: &str);
    fn push_markdown(&mut self, tag: &str);

    fn set_paragraph_active(&mut self, active: bool);
    fn paragraph_active(&self) -> bool;

    fn set_bullet_list_active(&mut self, active: bool);
    fn bullet_list_active(&self) -> bool;

    fn set_ordered_list_active(&mut self, active: bool);
    fn ordered_list_active(&self) -> bool;
    fn set_ordered_list_counter(&mut self, counter: u32);
    fn ordered_list_counter(&self) -> u32;

    fn set_blockquote_active(&mut self, active: bool);
    fn blockquote_active(&self) -> bool;

    fn short_description(&self) -> &str;
    fn set_short_description(&mut self, short_description: String);

    fn open_heading(&mut self, level: &str) {
        self.push_html(&format!("<h{}>", level));
        let level = level.parse::<u8>().unwrap_or(1);

        for _ in 0..level {
            self.push_markdown("#");
        }

        self.push_markdown(" ");
    }

    fn open_paragraph(&mut self) {
        self.push_html("<p>");
        self.set_paragraph_active(true);
    }

    fn open_bold(&mut self) {
        self.push_html("<strong>");
        self.push_markdown("**");
    }

    fn open_italic(&mut self) {
        self.push_html("<em>");
        self.push_markdown("_");
    }

    fn open_strike(&mut self) {
        self.push_html("<s>");
        self.push_markdown("~~");
    }

    fn open_code(&mut self) {
        self.push_html("<code spellcheck=\"false\">");
        self.push_markdown("`");
    }

    fn open_bullet_list(&mut self) {
        self.push_html("<ul>");
        self.set_bullet_list_active(true);
    }

    fn open_ordered_list(&mut self) {
        self.push_html("<ol>");
        self.set_ordered_list_active(true);
    }

    fn open_list_item(&mut self) {
        self.push_html("<li>");

        if self.ordered_list_active() {
            let counter = self.ordered_list_counter();

            self.push_markdown(&format!("{}. ", counter));
            self.set_ordered_list_counter(counter + 1);
        } else {
            self.push_markdown("* ");
        }
    }

    fn open_blockquote(&mut self) {
        self.push_html("<blockquote>");
        self.set_blockquote_active(true);
    }

    fn open_code_block(&mut self, language_tag: Option<&str>) {
        let mut data_lang = String::new();

        if let Some(language) = language_tag {
            data_lang = format!("data-code-block-language=\"{}\"", language);
        }

        let html = format!("<pre spellcheck=\"false\"<code {}>", data_lang);
        let markdown = format!("```{}\n", language_tag.unwrap_or_default());

        self.push_html(&html);
        self.push_markdown(&markdown);
    }

    fn open_image(&mut self, src: &str, alt: &str) {
        self.push_html(&format!(
            "<img alt=\"{}\" src=\"{}\" title=\"\" resizable=\"false\">",
            alt, src
        ));
        self.push_markdown(&format!("\n![{}]({})", alt, src));
    }

    fn open_link(&mut self, href: &str) {
        let link = &format!("<a href=\"{}\">", href);

        self.push_html(link);
        self.push_markdown(&format!("[{}](", href));
    }

    fn open_hard_break(&mut self) {
        self.push_html("<br/>");
    }

    fn text(&mut self, text: &str) -> Result<(), NodecosmosError> {
        self.push_html(&text);

        if self.blockquote_active() {
            self.push_markdown(&format!("> {}", text));
        } else {
            self.push_markdown(text);
        }

        if self.paragraph_active() && self.short_description().is_empty() {
            let mut short_description = text.to_string();
            short_description.truncate(Self::SHORT_DESCRIPTION_LENGTH - Self::ELLIPSIS.len());
            short_description = short_description.trim_end().to_string();

            if short_description.len() < text.len() {
                short_description.push_str(Self::ELLIPSIS);
            }

            self.set_short_description(short_description);
        }

        Ok(())
    }

    fn close_paragraph(&mut self) {
        if self.ordered_list_active() || self.bullet_list_active() {
            self.push_html("</p>");
            self.push_markdown("\n");
        } else if self.blockquote_active() {
            self.push_html("</p>");
            self.push_markdown("\n>");
        } else {
            self.push_html("</p>");
            self.push_markdown("\n");
        }

        self.set_paragraph_active(false);
    }

    fn close_bold(&mut self) {
        self.push_html("</strong>");
        self.push_markdown("** ");
    }

    fn close_italic(&mut self) {
        self.push_html("</em>");
        self.push_markdown("_ ");
    }

    fn close_strike(&mut self) {
        self.push_html("</s>");
        self.push_markdown("~~ ");
    }

    fn close_code(&mut self) {
        self.push_html("</code>");
        self.push_markdown("` ");
    }

    fn close_list_item(&mut self) {
        self.push_html("</li>");
        self.push_markdown("\n");
    }

    fn close_bullet_list(&mut self) {
        self.push_html("</ul>");
        self.push_markdown("\n");

        self.set_bullet_list_active(false);
    }

    fn close_ordered_list(&mut self) {
        self.push_html("</ol>");
        self.set_ordered_list_active(false);
        self.set_ordered_list_counter(1);
        self.push_markdown("\n");
    }

    fn close_blockquote(&mut self) {
        self.push_html("</blockquote>");
        self.push_markdown("\n");

        self.set_blockquote_active(false);
    }

    fn close_code_block(&mut self) {
        self.push_html("</code></pre>");
        self.push_markdown("\n```");
        self.push_markdown("\n");
    }

    fn close_image(&mut self) {
        self.push_markdown("\n");
    }

    fn close_link(&mut self) {
        self.push_html("</a>");
        self.push_markdown(")");
    }

    fn close_heading(&mut self, heading_level: &str) {
        self.push_html(&format!("</h{}>", heading_level));
        self.push_markdown("\n");
    }
}

#[derive(Clone, strum_macros::Display, strum_macros::EnumString)]
enum Tag {
    #[strum(serialize = "description")]
    Description,

    #[strum(serialize = "heading")]
    Heading,

    #[strum(serialize = "paragraph")]
    Paragraph,

    #[strum(serialize = "bold")]
    Bold,

    #[strum(serialize = "italic")]
    Italic,

    #[strum(serialize = "strike")]
    Strike,

    #[strum(serialize = "code")]
    Code,

    #[strum(serialize = "bulletList")]
    BulletList,

    #[strum(serialize = "orderedList")]
    OrderedList,

    #[strum(serialize = "listItem")]
    ListItem,

    #[strum(serialize = "blockquote")]
    Blockquote,

    #[strum(serialize = "codeBlock")]
    CodeBlock,

    #[strum(serialize = "image")]
    Image,

    #[strum(serialize = "link")]
    Link,

    #[strum(serialize = "hardBreak")]
    HardBreak,

    #[strum(serialize = "html")]
    Html,

    #[strum(serialize = "unknown")]
    Unknown,
}

impl<'a> From<QName<'a>> for Tag {
    fn from(value: QName<'a>) -> Self {
        Tag::from_str(std::str::from_utf8(value.as_ref()).unwrap_or_default()).unwrap_or(Tag::Unknown)
    }
}

impl From<&Arc<str>> for Tag {
    fn from(value: &Arc<str>) -> Self {
        Tag::from_str(value.as_ref()).unwrap_or(Tag::Unknown)
    }
}

pub struct DescriptionXmlParser<'a> {
    pub html: String,
    pub markdown: String,
    pub short_description: String,
    reader: Reader<&'a [u8]>,
    heading_level: String,
    ordered_list_counter: u32,
    paragraph_active: bool,
    ordered_list_active: bool,
    bullet_list_active: bool,
    blockquote_active: bool,
}

// this implementation should have method for each event
impl<'a> DescriptionXmlParser<'a> {
    pub fn new(xml: &'a str) -> Self {
        let reader = Reader::from_str(xml);

        Self {
            html: String::new(),
            markdown: String::new(),
            short_description: String::new(),
            reader,
            heading_level: String::new(),
            ordered_list_counter: 1,
            paragraph_active: false,
            ordered_list_active: false,
            bullet_list_active: false,
            blockquote_active: false,
        }
    }

    pub fn run(mut self) -> Result<Self, NodecosmosError> {
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(ref e)) => match Tag::from(e.name()) {
                    Tag::Description => (),
                    Tag::Heading => {
                        let heading_level = e
                            .attributes()
                            .find_map(|a| {
                                a.ok().filter(|a| a.key == QName(b"level")).map(|a| {
                                    a.decode_and_unescape_value(self.reader.decoder())
                                        .unwrap_or_else(|_| Cow::from("1".to_string()))
                                        .to_string()
                                })
                            })
                            .unwrap_or_else(|| "1".to_string());
                        self.open_heading(&heading_level);
                        self.heading_level = heading_level;
                    }
                    Tag::Paragraph => self.open_paragraph(),
                    Tag::Bold => self.open_bold(),
                    Tag::Italic => self.open_italic(),
                    Tag::Strike => self.open_strike(),
                    Tag::Code => self.open_code(),
                    Tag::BulletList => self.open_bullet_list(),
                    Tag::OrderedList => self.open_ordered_list(),
                    Tag::ListItem => self.open_list_item(),
                    Tag::Blockquote => self.open_blockquote(),
                    Tag::CodeBlock => {
                        let language_tag = e.attributes().find_map(|a| {
                            a.ok()
                                .filter(|a| a.key == QName(b"language"))
                                .map(|a| a.decode_and_unescape_value(self.reader.decoder()).unwrap_or_default())
                        });

                        self.open_code_block(language_tag.as_deref());
                    }
                    Tag::Image => {
                        let src = e
                            .attributes()
                            .find_map(|a| {
                                a.ok()
                                    .filter(|a| a.key == QName(b"src"))
                                    .map(|a| a.decode_and_unescape_value(self.reader.decoder()).unwrap_or_default())
                            })
                            .unwrap_or_default();
                        let alt = e
                            .attributes()
                            .find_map(|a| {
                                a.ok()
                                    .filter(|a| a.key == QName(b"alt"))
                                    .map(|a| a.decode_and_unescape_value(self.reader.decoder()).unwrap_or_default())
                            })
                            .unwrap_or_default();

                        self.open_image(&src, &alt);
                    }
                    Tag::Link => {
                        let href = e
                            .attributes()
                            .find_map(|a| {
                                a.ok()
                                    .filter(|a| a.key == QName(b"href"))
                                    .map(|a| a.decode_and_unescape_value(self.reader.decoder()).unwrap_or_default())
                            })
                            .unwrap_or_default();

                        self.open_link(&href);
                    }
                    Tag::HardBreak => self.open_hard_break(),
                    Tag::Html => (),
                    Tag::Unknown => (),
                },
                Ok(Event::Text(e)) => {
                    let text_ref = e.as_ref();
                    let text = std::str::from_utf8(text_ref).unwrap_or_default();

                    self.text(text)?;
                }
                Ok(Event::End(ref e)) => match Tag::from(e.name()) {
                    Tag::Description => (),
                    Tag::Heading => self.close_heading(&self.heading_level.clone()),
                    Tag::Paragraph => self.close_paragraph(),
                    Tag::Bold => self.close_bold(),
                    Tag::Italic => self.close_italic(),
                    Tag::Strike => self.close_strike(),
                    Tag::Code => self.close_code(),
                    Tag::ListItem => self.close_list_item(),
                    Tag::BulletList => self.close_bullet_list(),
                    Tag::OrderedList => self.close_ordered_list(),
                    Tag::Blockquote => self.close_blockquote(),
                    Tag::CodeBlock => self.close_code_block(),
                    Tag::Image => self.close_image(),
                    Tag::Link => self.close_link(),
                    Tag::HardBreak => (),
                    Tag::Html => (),
                    Tag::Unknown => (),
                },
                Ok(Event::Eof) => {
                    break;
                }
                _ => (),
            }
        }

        Ok(self)
    }
}

impl<'a> DescriptionParser<'a> for DescriptionXmlParser<'a> {
    fn push_html(&mut self, tag: &str) {
        self.html.push_str(tag);
    }

    fn push_markdown(&mut self, tag: &str) {
        self.markdown.push_str(tag);
    }

    fn set_paragraph_active(&mut self, active: bool) {
        self.paragraph_active = active;
    }

    fn paragraph_active(&self) -> bool {
        self.paragraph_active
    }

    fn set_bullet_list_active(&mut self, active: bool) {
        self.bullet_list_active = active;
    }

    fn bullet_list_active(&self) -> bool {
        self.bullet_list_active
    }

    fn set_ordered_list_active(&mut self, active: bool) {
        self.ordered_list_active = active;
    }

    fn ordered_list_active(&self) -> bool {
        self.ordered_list_active
    }

    fn set_ordered_list_counter(&mut self, counter: u32) {
        self.ordered_list_counter = counter;
    }

    fn ordered_list_counter(&self) -> u32 {
        self.ordered_list_counter
    }

    fn set_blockquote_active(&mut self, active: bool) {
        self.blockquote_active = active;
    }

    fn blockquote_active(&self) -> bool {
        self.blockquote_active
    }

    fn short_description(&self) -> &str {
        &self.short_description
    }

    fn set_short_description(&mut self, short_description: String) {
        self.short_description = short_description;
    }
}

pub struct DescriptionYDocParser {
    pub html: String,
    pub markdown: String,
    pub short_description: String,
    ordered_list_counter: u32,
    paragraph_active: bool,
    ordered_list_active: bool,
    bullet_list_active: bool,
    blockquote_active: bool,
}

impl DescriptionYDocParser {
    pub fn new() -> Self {
        Self {
            html: String::new(),
            markdown: String::new(),
            short_description: String::new(),
            ordered_list_counter: 1,
            paragraph_active: false,
            ordered_list_active: false,
            bullet_list_active: false,
            blockquote_active: false,
        }
    }

    pub fn run(mut self, txn: &TransactionMut, xml_fragment: XmlFragmentRef) -> Result<Self, NodecosmosError> {
        self.traverse_children(txn, xml_fragment)?;

        Ok(self)
    }

    fn traverse_children<T: XmlFragment>(
        &mut self,
        txn: &TransactionMut,
        xml_fragment: T,
    ) -> Result<&mut Self, NodecosmosError> {
        for child in xml_fragment.children(txn) {
            match child {
                XmlOut::Element(element) => {
                    println!("element: {:?}", element.tag());

                    match Tag::from(element.tag()) {
                        Tag::Description => {}
                        Tag::Heading => {
                            let heading_level = element
                                .get_attribute::<TransactionMut>(txn, "level")
                                .unwrap_or_else(|| "1".to_string());

                            self.open_heading(&heading_level);
                            self.traverse_children(txn, element)?;
                            self.close_heading(&heading_level);
                        }
                        Tag::Paragraph => {
                            self.open_paragraph();
                            self.traverse_children(txn, element)?;
                            self.close_paragraph();
                        }
                        Tag::Bold => {
                            self.open_bold();
                            self.traverse_children(txn, element)?;
                            self.close_bold();
                        }
                        Tag::Italic => {
                            self.open_italic();
                            self.traverse_children(txn, element)?;
                            self.close_italic();
                        }
                        Tag::Strike => {
                            self.open_strike();
                            self.traverse_children(txn, element)?;
                            self.close_strike();
                        }
                        Tag::Code => {
                            self.open_code();
                            self.traverse_children(txn, element)?;
                            self.close_code();
                        }
                        Tag::BulletList => {
                            self.open_bullet_list();
                            self.traverse_children(txn, element)?;
                            self.close_bullet_list();
                        }
                        Tag::OrderedList => {
                            self.open_ordered_list();
                            self.traverse_children(txn, element)?;
                            self.close_ordered_list();
                        }
                        Tag::ListItem => {
                            self.open_list_item();
                            self.traverse_children(txn, element)?;
                            self.close_list_item();
                        }
                        Tag::Blockquote => {
                            self.open_blockquote();
                            self.traverse_children(txn, element)?;
                            self.close_blockquote();
                        }
                        Tag::CodeBlock => {
                            let language_tag = element.get_attribute::<TransactionMut>(txn, "language");
                            self.open_code_block(language_tag.as_deref());
                            self.traverse_children(txn, element)?;
                            self.close_code_block();
                        }
                        Tag::Image => {
                            let src = element.get_attribute::<TransactionMut>(txn, "src").unwrap_or_default();
                            let alt = element.get_attribute::<TransactionMut>(txn, "alt").unwrap_or_default();

                            self.open_image(&src, &alt);
                            self.traverse_children(txn, element)?;
                            self.close_image();
                        }
                        Tag::Link => {
                            let href = element.get_attribute::<TransactionMut>(txn, "href").unwrap_or_default();

                            self.open_link(&href);
                            self.traverse_children(txn, element)?;
                            self.close_link();
                        }
                        Tag::HardBreak => {
                            self.open_hard_break();
                        }
                        Tag::Html => (),
                        Tag::Unknown => {
                            log::error!("Unknown tag: {}", element.tag());
                        }
                    };
                }
                XmlOut::Text(text) => {
                    let text = text.get_string(txn);

                    println!("text: {:?}", text);

                    let escaped = quick_xml::escape::escape(&text);

                    self.text(&escaped)?;
                }
                _ => panic!("Unexpected XML fragment type"),
            }
        }

        Ok(self)
    }
}

impl<'a> DescriptionParser<'a> for DescriptionYDocParser {
    fn push_html(&mut self, str: &str) {
        self.html.push_str(str);
    }

    fn push_markdown(&mut self, str: &str) {
        self.markdown.push_str(str);
    }

    fn set_paragraph_active(&mut self, active: bool) {
        self.paragraph_active = active;
    }

    fn paragraph_active(&self) -> bool {
        self.paragraph_active
    }

    fn set_bullet_list_active(&mut self, active: bool) {
        self.bullet_list_active = active;
    }

    fn bullet_list_active(&self) -> bool {
        self.bullet_list_active
    }

    fn set_ordered_list_active(&mut self, active: bool) {
        self.ordered_list_active = active;
    }

    fn ordered_list_active(&self) -> bool {
        self.ordered_list_active
    }

    fn set_ordered_list_counter(&mut self, counter: u32) {
        self.ordered_list_counter = counter;
    }

    fn ordered_list_counter(&self) -> u32 {
        self.ordered_list_counter
    }

    fn set_blockquote_active(&mut self, active: bool) {
        self.blockquote_active = active;
    }

    fn blockquote_active(&self) -> bool {
        self.blockquote_active
    }

    fn short_description(&self) -> &str {
        &self.short_description
    }

    fn set_short_description(&mut self, short_description: String) {
        self.short_description = short_description;
    }
}
