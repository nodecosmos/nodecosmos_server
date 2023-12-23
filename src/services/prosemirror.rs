use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use serde::de::Unexpected::Option;
use std::borrow::Cow;

pub struct ProseMirrorDoc {
    pub html: String,
    pub markdown: String,
}

impl ProseMirrorDoc {
    pub fn from_xml(xml: &str) -> Self {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut html = String::new();
        let mut markdown = String::new();
        let mut heading_level = String::new(); // To store the heading level
        let mut ordered_list_active = false;
        let mut bullet_list_active = false;
        let mut blockquote_active = false;
        let mut ordered_list_counter = 1;
        let mut current_href = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => match e.name() {
                    QName(b"heading") => {
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
                    QName(b"paragraph") => {
                        html.push_str("<p>");
                    }
                    QName(b"bold") => {
                        html.push_str("<strong>");
                        markdown.push_str("**");
                    }
                    QName(b"italic") => {
                        html.push_str("<em>");
                        markdown.push_str("_");
                    }
                    QName(b"strike") => {
                        html.push_str("<s>");
                        markdown.push_str("~~");
                    }
                    QName(b"code") => {
                        html.push_str("<code>");
                        markdown.push_str("`");
                    }
                    QName(b"bulletList") => {
                        html.push_str("<ul>");
                        bullet_list_active = true;
                    }
                    QName(b"orderedList") => {
                        html.push_str("<ol>");
                        ordered_list_active = true;
                    }
                    QName(b"listItem") => {
                        html.push_str("<li>");

                        if ordered_list_active {
                            markdown.push_str(&format!("{}. ", ordered_list_counter));
                            ordered_list_counter += 1;
                        } else {
                            markdown.push_str("* ");
                        }
                    }
                    QName(b"blockquote") => {
                        html.push_str("<blockquote>");
                        blockquote_active = true;
                    }
                    QName(b"codeBlock") => {
                        html.push_str("<pre>");
                        markdown.push_str("```markdown\n");
                    }
                    QName(b"img") => {
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
                    QName(b"link") => {
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
                    _ => (),
                },
                Ok(Event::Text(e)) => {
                    let text = e.unescape().unwrap();

                    html.push_str(&text);

                    if blockquote_active {
                        markdown.push_str(&format!("> {}", text));
                    } else {
                        markdown.push_str(&text);
                    }
                }
                Ok(Event::End(ref e)) => match e.name() {
                    QName(b"heading") => {
                        html.push_str(&format!("</h{}>", heading_level));
                        markdown.push_str("\n\n");
                    }
                    QName(b"paragraph") => {
                        html.push_str("</p>");
                        if ordered_list_active || bullet_list_active {
                            markdown.push_str("\n");
                        } else if blockquote_active {
                            markdown.push_str("\n>\n");
                        } else {
                            markdown.push_str("\n\n");
                        }
                    }
                    QName(b"bold") => {
                        html.push_str("</strong>");
                        markdown.push_str("** ");
                    }
                    QName(b"italic") => {
                        html.push_str("</em>");
                        markdown.push_str("_ ");
                    }
                    QName(b"strike") => {
                        html.push_str("</s>");
                        markdown.push_str("~~ ");
                    }
                    QName(b"code") => {
                        html.push_str("</code>");
                        markdown.push_str("` ");
                    }
                    QName(b"listItem") => {
                        html.push_str("</li>");
                        markdown.push_str("\n");
                    }
                    QName(b"bulletList") => {
                        html.push_str("</ul>");
                        markdown.push_str("\n");
                        bullet_list_active = false;
                    }
                    QName(b"orderedList") => {
                        html.push_str("</ol>");
                        ordered_list_active = false;
                        ordered_list_counter = 1;
                        markdown.push_str("\n");
                    }
                    QName(b"blockquote") => {
                        html.push_str("</blockquote>");
                        blockquote_active = false;
                        markdown.push_str("\n");
                    }
                    QName(b"codeBlock") => {
                        html.push_str("</pre>");
                        markdown.push_str("\n```");
                        markdown.push_str("\n\n");
                    }
                    QName(b"img") => {
                        markdown.push_str("\n\n");
                    }
                    QName(b"link") => {
                        html.push_str("</a>");
                        markdown.push_str(&format!("]({})", current_href));
                        current_href = String::new();
                    }
                    _ => (),
                },
                Ok(Event::Eof) => break,
                _ => (),
            }
        }

        Self { html, markdown }
    }
}
