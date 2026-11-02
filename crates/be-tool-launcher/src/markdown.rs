use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, parse_document};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Block {
    Heading(String),
    Paragraph(String),
    Code(String),
    Quote(String),
    Item(String),
    Image { url: String, alt: String },
    Table(Vec<Vec<Vec<Block>>>),
    Rule,
}

pub(crate) fn images(blocks: &[Block], found: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Image { url, .. } => found.push(url.clone()),
            Block::Table(rows) => {
                for cells in rows {
                    for cell in cells {
                        images(cell, found);
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn blocks(markdown: &str) -> Vec<Block> {
    parse(markdown, true)
}

fn parse(markdown: &str, html: bool) -> Vec<Block> {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    let root = parse_document(&arena, markdown, &options);
    let mut found = Vec::new();
    for child in root.children() {
        block(child, html, &mut found);
    }
    found
}

fn block<'a>(node: &'a AstNode<'a>, html: bool, found: &mut Vec<Block>) {
    let value = node.data().value.clone();
    match value {
        NodeValue::Paragraph => inline(node, Block::Paragraph, found),
        NodeValue::Heading(_) => inline(node, Block::Heading, found),
        NodeValue::CodeBlock(code) => {
            found.push(Block::Code(code.literal.trim_end_matches('\n').to_owned()));
        }
        NodeValue::ThematicBreak => found.push(Block::Rule),
        NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) | NodeValue::Alert(_) => {
            for child in node.children() {
                match child.data().value {
                    NodeValue::Paragraph => inline(child, Block::Quote, found),
                    _ => block(child, html, found),
                }
            }
        }
        NodeValue::Item(_) | NodeValue::TaskItem(_) => {
            for child in node.children() {
                match child.data().value {
                    NodeValue::Paragraph => inline(child, Block::Item, found),
                    _ => block(child, html, found),
                }
            }
        }
        NodeValue::Table(_) => {
            let rows = node
                .children()
                .map(|row| {
                    row.children()
                        .map(|cell| {
                            let mut blocks = Vec::new();
                            inline(cell, Block::Paragraph, &mut blocks);
                            blocks
                        })
                        .collect()
                })
                .collect();
            found.push(Block::Table(rows));
        }
        NodeValue::HtmlBlock(block) if html => {
            found.extend(parse(
                &html_breaks(&without_comments(&block.literal)),
                false,
            ));
        }
        NodeValue::HtmlBlock(block) => raw_html(&block.literal, found),
        _ => {
            for child in node.children() {
                self::block(child, html, found);
            }
        }
    }
}

fn inline<'a>(node: &'a AstNode<'a>, wrap: fn(String) -> Block, found: &mut Vec<Block>) {
    let mut text = String::new();
    gather(node, wrap, &mut text, found);
    flush(&mut text, wrap, found);
}

fn gather<'a>(
    node: &'a AstNode<'a>,
    wrap: fn(String) -> Block,
    text: &mut String,
    found: &mut Vec<Block>,
) {
    for child in node.children() {
        let value = child.data().value.clone();
        match value {
            NodeValue::Text(literal) => text.push_str(&literal),
            NodeValue::Code(code) => text.push_str(&code.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => text.push(' '),
            NodeValue::Image(link) => {
                flush(text, wrap, found);
                let mut alt = String::new();
                gather(child, wrap, &mut alt, &mut Vec::new());
                found.push(Block::Image {
                    url: link.url.clone(),
                    alt: alt.trim().to_owned(),
                });
            }
            NodeValue::HtmlInline(tag) => {
                if let Some((_, _, url, alt)) = html_image(&tag) {
                    flush(text, wrap, found);
                    found.push(Block::Image { url, alt });
                } else if tag.to_ascii_lowercase().starts_with("<br") {
                    text.push(' ');
                }
            }
            _ => gather(child, wrap, text, found),
        }
    }
}

fn flush(text: &mut String, wrap: fn(String) -> Block, found: &mut Vec<Block>) {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.clear();
    if !collapsed.is_empty() {
        found.push(wrap(collapsed));
    }
}

fn raw_html(html: &str, found: &mut Vec<Block>) {
    let mut rest = html;
    while let Some((start, end, url, alt)) = html_image(rest) {
        plain_text(&rest[..start], found);
        found.push(Block::Image { url, alt });
        rest = &rest[end..];
    }
    plain_text(rest, found);
}

fn plain_text(html: &str, found: &mut Vec<Block>) {
    let mut text = String::new();
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            _ => text.push(character),
        }
    }
    let text = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    let mut text = text;
    flush(&mut text, Block::Paragraph, found);
}

fn without_comments(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        kept.push_str(&rest[..start]);
        rest = match rest[start..].find("-->") {
            Some(end) => &rest[start + end + 3..],
            None => "",
        };
    }
    kept.push_str(rest);
    kept
}

fn html_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tag = &rest[open..];
        let Some(close) = tag.find('>') else {
            rest = tag;
            break;
        };
        let inner = tag[1..close].trim_start_matches('/').to_ascii_lowercase();
        let name = inner
            .split(|character: char| character.is_whitespace() || character == '/')
            .next()
            .unwrap_or_default();
        match name {
            "li" if !tag[1..].starts_with('/') => out.push_str("\n\n- "),
            "p" | "br" | "div" | "ul" | "ol" | "li" | "details" | "summary" | "blockquote"
            | "table" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => out.push_str("\n\n"),
            "td" | "th" => out.push(' '),
            _ => out.push_str(&tag[..=close]),
        }
        rest = &tag[close + 1..];
    }
    out.push_str(rest);
    out
}

fn html_image(text: &str) -> Option<(usize, usize, String, String)> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<img")?;
    let end = start + lower[start..].find('>')? + 1;
    let tag = &text[start..end];
    let url = attribute(tag, "src")?;
    let alt = attribute(tag, "alt").unwrap_or_default();
    Some((start, end, url, alt))
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(name) {
        let at = from + offset;
        let before = lower[..at].chars().last();
        let rest = lower[at + name.len()..].trim_start();
        if before.is_some_and(char::is_whitespace)
            && let Some(value) = rest.strip_prefix('=')
        {
            let value = value.trim_start();
            let offset = tag.len() - value.len();
            let quote = value.chars().next()?;
            return if quote == '"' || quote == '\'' {
                let inner = &tag[offset + 1..];
                inner.find(quote).map(|end| inner[..end].to_owned())
            } else {
                let inner = &tag[offset..];
                let end = inner
                    .find(|character: char| character.is_whitespace() || character == '>')
                    .unwrap_or(inner.len());
                Some(inner[..end].trim_end_matches('/').to_owned())
            };
        }
        from = at + name.len();
    }
    None
}
