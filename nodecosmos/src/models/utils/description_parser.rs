use quick_xml::events::{BytesText, Event};
use quick_xml::name::QName;
use quick_xml::Reader;

use crate::errors::NodecosmosError;

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
            _ => panic!("Unknown tag"),
        }
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
    code_block_active: bool,
}

// this implementation should have method for each event
impl<'a> DescriptionXmlParser<'a> {
    const SHORT_DESCRIPTION_LENGTH: usize = 255;
    const ELLIPSIS: &'static str = "...";

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
            code_block_active: false,
        }
    }

    pub fn run(mut self) -> Result<Self, NodecosmosError> {
        loop {
            match self.reader.read_event() {
                Ok(Event::Start(ref e)) => match ProseMirrorXmlTag::from(e.name()) {
                    ProseMirrorXmlTag::Heading => self.open_heading(&e),
                    ProseMirrorXmlTag::Paragraph => self.open_paragraph(),
                    ProseMirrorXmlTag::Bold => self.open_bold(),
                    ProseMirrorXmlTag::Italic => self.open_italic(),
                    ProseMirrorXmlTag::Strike => self.open_strike(),
                    ProseMirrorXmlTag::Code => self.open_code(),
                    ProseMirrorXmlTag::BulletList => self.open_bullet_list(),
                    ProseMirrorXmlTag::OrderedList => self.open_ordered_list(),
                    ProseMirrorXmlTag::ListItem => self.open_list_item(),
                    ProseMirrorXmlTag::Blockquote => self.open_blockquote(),
                    ProseMirrorXmlTag::CodeBlock => self.open_code_block(),
                    ProseMirrorXmlTag::Image => self.open_image(&e),
                    ProseMirrorXmlTag::Link => self.open_link(&e),
                    ProseMirrorXmlTag::HardBreak => self.open_hard_break(),
                    ProseMirrorXmlTag::Html => (),
                },
                Ok(Event::Text(e)) => self.text(e)?,
                Ok(Event::End(ref e)) => match ProseMirrorXmlTag::from(e.name()) {
                    ProseMirrorXmlTag::Heading => self.close_heading(),
                    ProseMirrorXmlTag::Paragraph => self.close_paragraph(),
                    ProseMirrorXmlTag::Bold => self.close_bold(),
                    ProseMirrorXmlTag::Italic => self.close_italic(),
                    ProseMirrorXmlTag::Strike => self.close_strike(),
                    ProseMirrorXmlTag::Code => self.close_code(),
                    ProseMirrorXmlTag::ListItem => self.close_list_item(),
                    ProseMirrorXmlTag::BulletList => self.close_bullet_list(),
                    ProseMirrorXmlTag::OrderedList => self.close_ordered_list(),
                    ProseMirrorXmlTag::Blockquote => self.close_blockquote(),
                    ProseMirrorXmlTag::CodeBlock => self.close_code_block(),
                    ProseMirrorXmlTag::Image => self.close_image(),
                    ProseMirrorXmlTag::Link => self.close_link(),
                    ProseMirrorXmlTag::HardBreak => (),
                    ProseMirrorXmlTag::Html => (),
                },
                Ok(Event::Eof) => {
                    break;
                }
                _ => (),
            }
        }

        Ok(self)
    }

    fn open_heading(&mut self, event: &quick_xml::events::BytesStart) {
        self.heading_level = event
            .attributes()
            .find_map(|a| {
                a.ok().filter(|a| a.key == QName(b"level")).map(|a| {
                    a.decode_and_unescape_value(self.reader.decoder())
                        .expect("Error decoding heading level")
                        .to_string()
                })
            })
            .unwrap_or_else(|| "1".to_string());
        self.html.push_str(&format!("<h{}>", self.heading_level));
        let level = self.heading_level.parse::<u8>().expect("Error parsing heading level");

        for _ in 0..level {
            self.markdown.push('#');
        }
        self.markdown.push(' ');
    }

    fn open_paragraph(&mut self) {
        self.html.push_str("<p>");
        self.paragraph_active = true;
    }

    fn open_bold(&mut self) {
        self.html.push_str("<strong>");
        self.markdown.push_str("**");
    }

    fn open_italic(&mut self) {
        self.html.push_str("<em>");
        self.markdown.push_str("_");
    }

    fn open_strike(&mut self) {
        self.html.push_str("<s>");
        self.markdown.push_str("~~");
    }

    fn open_code(&mut self) {
        self.html.push_str("<code spellcheck=\"false\">");
        self.markdown.push_str("`");
    }

    fn open_bullet_list(&mut self) {
        self.html.push_str("<ul>");
        self.bullet_list_active = true;
    }

    fn open_ordered_list(&mut self) {
        self.html.push_str("<ol>");
        self.ordered_list_active = true;
    }

    fn open_list_item(&mut self) {
        self.html.push_str("<li>");

        if self.ordered_list_active {
            self.markdown.push_str(&format!("{}. ", self.ordered_list_counter));
            self.ordered_list_counter += 1;
        } else {
            self.markdown.push_str("* ");
        }
    }

    fn open_blockquote(&mut self) {
        self.html.push_str("<blockquote>");
        self.blockquote_active = true;
    }

    fn open_code_block(&mut self) {
        self.html
            .push_str("<pre spellcheck=\"false\" class=\"language-markup\"><code data-code-block-language=\"markup\">");
        self.markdown.push_str("```markup\n");

        self.code_block_active = true;
    }

    fn open_image(&mut self, event: &quick_xml::events::BytesStart) {
        let src = event
            .attributes()
            .find_map(|a| {
                a.ok()
                    .filter(|a| a.key == QName(b"src"))
                    .map(|a| a.decode_and_unescape_value(self.reader.decoder()).unwrap())
            })
            .unwrap_or_default();
        let alt = event
            .attributes()
            .find_map(|a| {
                a.ok()
                    .filter(|a| a.key == QName(b"alt"))
                    .map(|a| a.decode_and_unescape_value(self.reader.decoder()).unwrap())
            })
            .unwrap_or_default();
        self.html.push_str(&format!(
            "<img alt=\"{}\" src=\"{}\" title=\"\" resizable=\"false\">",
            alt, src
        ));
        self.markdown.push_str(&format!("\n![{}]({})", alt, src));
    }

    fn open_link(&mut self, event: &quick_xml::events::BytesStart) {
        let href = event
            .attributes()
            .find_map(|a| {
                a.ok()
                    .filter(|a| a.key == QName(b"href"))
                    .map(|a| a.decode_and_unescape_value(self.reader.decoder()).unwrap())
            })
            .unwrap_or_default();

        let link = &format!("<a href=\"{}\">", href);

        self.html.push_str(link);
        self.markdown.push_str(link);
    }

    fn open_hard_break(&mut self) {
        self.html.push_str("<br/>");
        self.markdown.push_str("\n\n");
    }

    fn text(&mut self, event: BytesText<'a>) -> Result<(), NodecosmosError> {
        let text = event.unescape()?;

        self.html.push_str(&text);

        if self.blockquote_active {
            self.markdown.push_str(&format!("> {}", text));
        } else {
            self.markdown.push_str(&text);
        }

        if self.paragraph_active && self.short_description.is_empty() {
            self.short_description = text.to_string();
            self.short_description
                .truncate(Self::SHORT_DESCRIPTION_LENGTH - Self::ELLIPSIS.len());
            self.short_description = self.short_description.trim_end().to_string();

            if self.short_description.len() < text.len() {
                self.short_description.push_str(Self::ELLIPSIS);
            }
        }

        Ok(())
    }

    fn close_heading(&mut self) {
        self.html.push_str(&format!("</h{}>", self.heading_level));
        self.markdown.push_str("\n\n");
    }

    fn close_paragraph(&mut self) {
        if self.ordered_list_active || self.bullet_list_active {
            self.html.push_str("</p>");
            self.markdown.push_str("\n");
        } else if self.blockquote_active {
            self.html.push_str("</p>");
            self.markdown.push_str("\n>\n");
        } else {
            self.html.push_str("</p>");
            self.markdown.push_str("\n\n");
        }

        self.paragraph_active = false;
    }

    fn close_bold(&mut self) {
        self.html.push_str("</strong>");
        self.markdown.push_str("** ");
    }

    fn close_italic(&mut self) {
        self.html.push_str("</em>");
        self.markdown.push_str("_ ");
    }

    fn close_strike(&mut self) {
        self.html.push_str("</s>");
        self.markdown.push_str("~~ ");
    }

    fn close_code(&mut self) {
        self.html.push_str("</code>");
        self.markdown.push_str("` ");
    }

    fn close_list_item(&mut self) {
        self.html.push_str("</li>");
        self.markdown.push_str("\n");
    }

    fn close_bullet_list(&mut self) {
        self.html.push_str("</ul>");
        self.markdown.push_str("\n");

        self.bullet_list_active = false;
    }

    fn close_ordered_list(&mut self) {
        self.html.push_str("</ol>");
        self.ordered_list_active = false;
        self.ordered_list_counter = 1;
        self.markdown.push_str("\n");
    }

    fn close_blockquote(&mut self) {
        self.html.push_str("</blockquote>");
        self.markdown.push_str("\n");

        self.blockquote_active = false;
    }

    fn close_code_block(&mut self) {
        self.html.push_str("</code></pre>");
        self.markdown.push_str("\n```");
        self.markdown.push_str("\n\n");

        self.code_block_active = false;
    }

    fn close_image(&mut self) {
        self.markdown.push_str("\n\n");
    }

    fn close_link(&mut self) {
        self.html.push_str("</a>");
        self.markdown.push_str("</a>");
    }
}
