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
    let text = html_breaks(&without_comments(&markdown.replace('\r', "")));
    let mut parsed = Parsed::default();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if let Some(fence) = fence(trimmed) {
            parsed.flush();
            let mut code = Vec::new();
            for line in lines.by_ref() {
                if line.trim().starts_with(fence) {
                    break;
                }
                code.push(line);
            }
            parsed.blocks.push(Block::Code(code.join("\n")));
        } else if trimmed.starts_with('|') {
            parsed.flush();
            let mut rows = vec![trimmed];
            while let Some(next) = lines.clone().next().map(str::trim)
                && next.starts_with('|')
            {
                rows.push(next);
                lines.next();
            }
            parsed.blocks.push(table(&rows));
        } else if trimmed.is_empty() {
            parsed.flush();
        } else if is_rule(trimmed) {
            parsed.flush();
            parsed.blocks.push(Block::Rule);
        } else if let Some(heading) = heading(trimmed) {
            parsed.flush();
            parsed.inline(heading, Block::Heading);
        } else if let Some(quoted) = trimmed.strip_prefix('>') {
            parsed.continue_with(Kind::Quote, quoted.trim());
        } else if let Some(item) = item(trimmed) {
            parsed.flush();
            parsed.continue_with(Kind::Item, item);
        } else {
            let kind = match parsed.open {
                Some((Kind::Item, _)) if line.starts_with([' ', '\t']) => Kind::Item,
                _ => Kind::Paragraph,
            };
            parsed.continue_with(kind, trimmed);
        }
    }
    parsed.flush();
    parsed.blocks
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Paragraph,
    Quote,
    Item,
}

#[derive(Default)]
struct Parsed {
    blocks: Vec<Block>,
    open: Option<(Kind, String)>,
}

impl Parsed {
    fn continue_with(&mut self, kind: Kind, text: &str) {
        match &mut self.open {
            Some((open, collected)) if *open == kind && kind != Kind::Item => {
                collected.push(' ');
                collected.push_str(text);
            }
            Some((Kind::Item, collected)) if kind == Kind::Item && !collected.is_empty() => {
                collected.push(' ');
                collected.push_str(text);
            }
            _ => {
                self.flush();
                self.open = Some((kind, text.to_owned()));
            }
        }
    }

    fn flush(&mut self) {
        let Some((kind, text)) = self.open.take() else {
            return;
        };
        let wrap = match kind {
            Kind::Paragraph => Block::Paragraph,
            Kind::Quote => Block::Quote,
            Kind::Item => Block::Item,
        };
        self.inline(&text, wrap);
    }

    fn inline(&mut self, text: &str, wrap: fn(String) -> Block) {
        let mut rest = text;
        while let Some((start, end, url, alt)) = next_image(rest) {
            self.text(&rest[..start], wrap);
            self.blocks.push(Block::Image { url, alt });
            rest = &rest[end..];
        }
        self.text(rest, wrap);
    }

    fn text(&mut self, text: &str, wrap: fn(String) -> Block) {
        let cleaned = plain(text);
        if !cleaned.is_empty() {
            self.blocks.push(wrap(cleaned));
        }
    }
}

fn table(rows: &[&str]) -> Block {
    Block::Table(
        rows.iter()
            .map(|row| cells(row))
            .filter(|cells| !cells.iter().all(|cell| is_divider(cell)))
            .map(|cells| {
                cells
                    .into_iter()
                    .map(|cell| {
                        let mut parsed = Parsed::default();
                        parsed.inline(cell, Block::Paragraph);
                        parsed.blocks
                    })
                    .collect()
            })
            .collect(),
    )
}

fn cells(row: &str) -> Vec<&str> {
    let row = row.trim();
    let row = row.strip_prefix('|').unwrap_or(row);
    let row = row.strip_suffix('|').unwrap_or(row);
    row.split('|').map(str::trim).collect()
}

fn is_divider(cell: &str) -> bool {
    !cell.is_empty()
        && cell
            .chars()
            .all(|character| matches!(character, '-' | ':' | ' '))
}

fn fence(line: &str) -> Option<&'static str> {
    if line.starts_with("```") {
        Some("```")
    } else if line.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn is_rule(line: &str) -> bool {
    let compact: String = line.chars().filter(|character| *character != ' ').collect();
    compact.len() >= 3
        && ['-', '*', '_']
            .iter()
            .any(|marker| compact.chars().all(|character| character == *marker))
}

fn heading(line: &str) -> Option<&str> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if (1..=6).contains(&level) {
        line[level..].strip_prefix(' ').map(str::trim)
    } else {
        None
    }
}

fn item(line: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some(rest.trim());
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &line[digits..];
        if let Some(rest) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some(rest.trim());
        }
    }
    None
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
            _ => out.push_str(&tag[..=close]),
        }
        rest = &tag[close + 1..];
    }
    out.push_str(rest);
    out
}

fn next_image(text: &str) -> Option<(usize, usize, String, String)> {
    let markdown = markdown_image(text);
    let html = html_image(text);
    match (markdown, html) {
        (Some(markdown), Some(html)) if html.0 < markdown.0 => Some(html),
        (Some(markdown), _) => Some(markdown),
        (None, html) => html,
    }
}

fn markdown_image(text: &str) -> Option<(usize, usize, String, String)> {
    let mut from = 0;
    while let Some(offset) = text[from..].find("![") {
        let start = from + offset;
        let after = &text[start + 2..];
        if let Some(close) = after.find("](") {
            let alt = &after[..close];
            let target = &after[close + 2..];
            if let Some(end) = target.find(')') {
                let url = target[..end].split_whitespace().next().unwrap_or_default();
                let consumed = start + 2 + close + 2 + end + 1;
                return Some((start, consumed, url.to_owned(), alt.to_owned()));
            }
        }
        from = start + 2;
    }
    None
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

fn plain(text: &str) -> String {
    let linked = links_as_text(text);
    let mut plain = String::with_capacity(linked.len());
    let mut in_tag = false;
    let characters: Vec<char> = linked.chars().collect();
    for (index, &character) in characters.iter().enumerate() {
        let spaced = |at: Option<&char>| at.is_none_or(|neighbour| neighbour.is_whitespace());
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            '`' => {}
            '*' if !(spaced(
                index
                    .checked_sub(1)
                    .and_then(|before| characters.get(before)),
            ) && spaced(characters.get(index + 1))) => {}
            _ => plain.push(character),
        }
    }
    let plain = plain.replace("__", "").replace("~~", "");
    let plain = plain
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn links_as_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find("](") else {
            break;
        };
        let target = &after[close + 2..];
        let Some(end) = target.find(')') else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&after[..close]);
        rest = &target[end + 1..];
    }
    out.push_str(rest);
    out
}
