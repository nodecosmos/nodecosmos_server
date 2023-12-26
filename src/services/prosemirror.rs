use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde::de::Unexpected::Option;
use std::borrow::Cow;

// ProseMirror uses json schema to define the structure of the document.
// Yjs on front-end encodes the document in base64 and sends it to the backend.
// Yjs uses xml that matches json schema and here we parse the xml to markdown and html.
// Also, it seems that Yjs uses camelCase for the xml tags.
// https://prosemirror.net/docs/ref/#schema-basic
enum ProseMirrorXmlTag {
    Paragraph,
    Heading,
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
}

impl<'a> From<QName<'a>> for ProseMirrorXmlTag {
    fn from(value: QName<'a>) -> Self {
        match value {
            QName(b"paragraph") => ProseMirrorXmlTag::Paragraph,
            QName(b"heading") => ProseMirrorXmlTag::Heading,
            QName(b"bold") => ProseMirrorXmlTag::Bold,
            QName(b"italic") => ProseMirrorXmlTag::Italic,
            QName(b"strike") => ProseMirrorXmlTag::Strike,
            QName(b"code") => ProseMirrorXmlTag::Code,
            QName(b"bulletList") => ProseMirrorXmlTag::BulletList,
            QName(b"orderedList") => ProseMirrorXmlTag::OrderedList,
            QName(b"listItem") => ProseMirrorXmlTag::ListItem,
            QName(b"blockquote") => ProseMirrorXmlTag::Blockquote,
            QName(b"codeBlock") => ProseMirrorXmlTag::CodeBlock,
            QName(b"img") => ProseMirrorXmlTag::Image,
            QName(b"link") => ProseMirrorXmlTag::Link,
            QName(b"hardBreak") => ProseMirrorXmlTag::HardBreak,
            _ => panic!("Unknown tag {:?}", value),
        }
    }
}

pub struct ProseMirrorDoc {
    pub html: String,
    pub markdown: String,
    pub short_description: String,
}

impl ProseMirrorDoc {
    const SHORT_DESCRIPTION_LENGTH: usize = 255;
    const ELLIPSIS: &'static str = "...";

    pub fn from_xml(xml: &str) -> Self {
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

        reader.trim_text(true);

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => match ProseMirrorXmlTag::from(e.name()) {
                    ProseMirrorXmlTag::Heading => {
                        heading_level = e
                            .attributes()
                            .find_map(|a| {
                                a.ok()
                                    .filter(|a| a.key == QName(b"level"))
                                    .map(|a| a.decode_and_unescape_value(&reader).unwrap().to_string())
                            })
                            .unwrap_or_else(|| "1".to_string());
                        html.push_str(&format!("<h{}>", heading_level));
                        let level = heading_level.parse::<u8>().unwrap();

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
                        markdown.push_str("```markdown\n");
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
                        html.push_str("<br/>");
                        markdown.push_str("\n");
                    }
                },
                Ok(Event::Text(e)) => {
                    let text = e.unescape().unwrap();

                    html.push_str(&text);

                    if blockquote_active {
                        markdown.push_str(&format!("> {}", text));
                    } else {
                        markdown.push_str(&text);
                    }

                    if paragraph_active && short_description.is_empty() {
                        short_description = text.to_string();
                        short_description.truncate(Self::SHORT_DESCRIPTION_LENGTH - Self::ELLIPSIS.len());
                        short_description = short_description.trim_end().to_string();

                        if short_description.len() < text.len() {
                            short_description.push_str(Self::ELLIPSIS);
                        }

                        markdown.push_str(&format!(" ({})", short_description));
                    }
                }
                Ok(Event::End(ref e)) => match ProseMirrorXmlTag::from(e.name()) {
                    ProseMirrorXmlTag::Heading => {
                        html.push_str(&format!("</h{}>", heading_level));
                        markdown.push_str("\n\n");
                    }
                    ProseMirrorXmlTag::Paragraph => {
                        html.push_str("</p>");
                        if ordered_list_active || bullet_list_active {
                            markdown.push_str("\n");
                        } else if blockquote_active {
                            markdown.push_str("\n>\n");
                        } else {
                            markdown.push_str("\n\n");
                        }

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
                },
                Ok(Event::Eof) => break,
                _ => (),
            }
        }

        Self {
            html,
            markdown,
            short_description,
        }
    }
}
