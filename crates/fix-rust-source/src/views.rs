use crate::{parse, syntax_range, text};
use ra_ap_syntax::ast::AstNode;
use ra_ap_syntax::{NodeOrToken, SyntaxKind};
use std::error::Error;
use std::ops::Range;

const MAX_WIDTH: usize = 100;
const INDENT: usize = 4;

pub fn format_views(source: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let tokens = collect_tokens(source)?;
    let calls = scan_views(&tokens, 0, tokens.len());
    if calls.is_empty() {
        return Ok(source.to_vec());
    }
    let formatter = Formatter {
        source,
        tokens: &tokens,
    };
    let mut output = source.to_vec();
    for call in calls.iter().rev() {
        let line_start = source[..call.range.start]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let prefix = text(source, line_start..call.range.start);
        let column = prefix.chars().count();
        let indent = prefix.len() - prefix.trim_start_matches([' ', '\t']).len();
        let lines = formatter.view_lines(&call.roots, indent, column);
        output.splice(call.range.clone(), lines.join("\n").bytes());
    }
    Ok(output)
}

struct Token {
    kind: SyntaxKind,
    text: String,
    range: Range<usize>,
}

struct ViewCall {
    range: Range<usize>,
    roots: Vec<Child>,
}

enum Child {
    Element(Element),
    Expr(Range<usize>),
}

struct Element {
    tag: String,
    attributes: Vec<Attribute>,
    children: Option<Vec<Child>>,
}

struct Attribute {
    special: bool,
    key: String,
    value: Option<Value>,
}

enum Value {
    Braced(Range<usize>),
    Bare(Range<usize>),
}

fn collect_tokens(source: &[u8]) -> Result<Vec<Token>, Box<dyn Error>> {
    let tree = parse(source)?;
    Ok(tree
        .syntax()
        .descendants_with_tokens()
        .filter_map(NodeOrToken::into_token)
        .filter(|token| !matches!(token.kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT))
        .map(|token| Token {
            kind: token.kind(),
            text: token.text().to_owned(),
            range: syntax_range(token.text_range()),
        })
        .collect())
}

fn matching(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.kind {
            SyntaxKind::L_CURLY | SyntaxKind::L_PAREN | SyntaxKind::L_BRACK => depth += 1,
            SyntaxKind::R_CURLY | SyntaxKind::R_PAREN | SyntaxKind::R_BRACK => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn scan_views(tokens: &[Token], from: usize, to: usize) -> Vec<ViewCall> {
    let mut calls = Vec::new();
    let mut index = from;
    while index < to {
        if let Some(end) = view_call_end(tokens, index, to)
            && let Some(roots) = parse_view(&tokens[index + 3..end])
        {
            calls.push(ViewCall {
                range: tokens[index].range.start..tokens[end].range.end,
                roots,
            });
            index = end + 1;
            continue;
        }
        index += 1;
    }
    calls
}

fn view_call_end(tokens: &[Token], index: usize, to: usize) -> Option<usize> {
    let name = tokens.get(index)?;
    if name.kind != SyntaxKind::IDENT || name.text != "view" {
        return None;
    }
    if tokens.get(index + 1)?.kind != SyntaxKind::BANG {
        return None;
    }
    if tokens.get(index + 2)?.kind != SyntaxKind::L_CURLY {
        return None;
    }
    let end = matching(tokens, index + 2)?;
    (end < to).then_some(end)
}

fn parse_view(tokens: &[Token]) -> Option<Vec<Child>> {
    let mut parser = Parser { tokens, index: 0 };
    let roots = parser.children()?;
    (parser.index == tokens.len()).then_some(roots)
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
}

impl Parser<'_> {
    fn at(&self, offset: usize, expected: &str) -> bool {
        self.tokens
            .get(self.index + offset)
            .is_some_and(|token| token.text == expected)
    }

    fn eat(&mut self, expected: &str) -> bool {
        let found = self.at(0, expected);
        if found {
            self.index += 1;
        }
        found
    }

    fn expect(&mut self, expected: &str) -> Option<()> {
        self.eat(expected).then_some(())
    }

    fn identifier(&mut self) -> Option<String> {
        let token = self.tokens.get(self.index)?;
        let first = token.text.chars().next()?;
        if !first.is_alphabetic() && first != '_' {
            return None;
        }
        self.index += 1;
        Some(token.text.clone())
    }

    fn tag_path(&mut self) -> Option<String> {
        let mut path = self.identifier()?;
        while self.at(0, ":") && self.at(1, ":") {
            self.index += 2;
            path.push_str("::");
            path.push_str(&self.identifier()?);
        }
        Some(path)
    }

    fn group(&mut self) -> Option<Range<usize>> {
        let open = self.index;
        let close = matching(self.tokens, open)?;
        self.index = close + 1;
        if close == open + 1 {
            let empty = self.tokens[open].range.end;
            return Some(empty..empty);
        }
        Some(self.tokens[open + 1].range.start..self.tokens[close - 1].range.end)
    }

    fn bare_value(&mut self) -> Option<Range<usize>> {
        let start = self.index;
        while self.at(0, "&") || self.at(0, "-") || self.at(0, "!") || self.at(0, "mut") {
            self.index += 1;
        }
        let token = self.tokens.get(self.index)?;
        if is_literal(token.kind) {
            self.index += 1;
        } else {
            if self.at(0, ":") && self.at(1, ":") {
                self.index += 2;
            }
            self.identifier()?;
            while self.at(0, ":") && self.at(1, ":") {
                self.index += 2;
                self.identifier()?;
            }
            if self.at(0, "(") {
                let close = matching(self.tokens, self.index)?;
                self.index = close + 1;
            }
        }
        Some(self.tokens[start].range.start..self.tokens[self.index - 1].range.end)
    }

    fn children(&mut self) -> Option<Vec<Child>> {
        let mut children = Vec::new();
        while self.index < self.tokens.len() {
            if self.at(0, "<") && self.at(1, "/") {
                break;
            }
            children.push(self.child()?);
        }
        Some(children)
    }

    fn child(&mut self) -> Option<Child> {
        if self.at(0, "<") {
            return Some(Child::Element(self.element()?));
        }
        if self.at(0, "{") {
            return Some(Child::Expr(self.group()?));
        }
        None
    }

    fn element(&mut self) -> Option<Element> {
        self.expect("<")?;
        let tag = self.tag_path()?;
        let mut attributes = Vec::new();
        while !self.at(0, ">") && !self.at(0, "/") {
            if self.index >= self.tokens.len() {
                return None;
            }
            let special = self.eat("@");
            let key = self.identifier()?;
            let value = if self.eat("=") {
                if self.at(0, "{") {
                    Some(Value::Braced(self.group()?))
                } else {
                    Some(Value::Bare(self.bare_value()?))
                }
            } else {
                None
            };
            attributes.push(Attribute {
                special,
                key,
                value,
            });
        }
        if self.eat("/") {
            self.expect(">")?;
            return Some(Element {
                tag,
                attributes,
                children: None,
            });
        }
        self.expect(">")?;
        let children = self.children()?;
        self.expect("<")?;
        self.expect("/")?;
        if self.tag_path()? != tag {
            return None;
        }
        self.expect(">")?;
        Some(Element {
            tag,
            attributes,
            children: Some(children),
        })
    }
}

fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::INT_NUMBER
            | SyntaxKind::FLOAT_NUMBER
            | SyntaxKind::STRING
            | SyntaxKind::BYTE_STRING
            | SyntaxKind::C_STRING
            | SyntaxKind::CHAR
            | SyntaxKind::BYTE
            | SyntaxKind::TRUE_KW
            | SyntaxKind::FALSE_KW
    )
}

struct Formatter<'a> {
    source: &'a [u8],
    tokens: &'a [Token],
}

impl Formatter<'_> {
    fn view_lines(&self, roots: &[Child], indent: usize, column: usize) -> Vec<String> {
        if roots.is_empty() {
            return vec!["view! {}".to_owned()];
        }
        if let Some(single) = self.view_text(roots)
            && column + single.chars().count() <= MAX_WIDTH
        {
            return vec![single];
        }
        let mut lines = vec!["view! {".to_owned()];
        for root in roots {
            let parts = self.child_lines(root, indent + INDENT, indent + INDENT);
            push_block(&mut lines, indent + INDENT, parts);
        }
        lines.push(format!("{}}}", spaces(indent)));
        lines
    }

    fn child_lines(&self, child: &Child, indent: usize, column: usize) -> Vec<String> {
        match child {
            Child::Element(element) => self.element_lines(element, indent, column),
            Child::Expr(range) => brace(self.expr_lines(range, indent, column + 1)),
        }
    }

    fn element_lines(&self, element: &Element, indent: usize, column: usize) -> Vec<String> {
        if let Some(single) = self.element_text(element)
            && column + single.chars().count() <= MAX_WIDTH
        {
            return vec![single];
        }
        let (inline_close, broken_close) = match element.children.as_deref() {
            None => (" />".to_owned(), "/>".to_owned()),
            Some([]) => {
                let empty = format!("></{}>", element.tag);
                (empty.clone(), empty)
            }
            Some(_) => (">".to_owned(), ">".to_owned()),
        };
        let mut lines = Vec::new();
        let inline = self
            .attributes_text(element)
            .map(|head| format!("{head}{inline_close}"))
            .filter(|head| column + head.chars().count() <= MAX_WIDTH);
        match inline {
            Some(head) => lines.push(head),
            None => {
                lines.push(format!("<{}", element.tag));
                for attribute in &element.attributes {
                    let parts = self.attribute_lines(attribute, indent + INDENT, indent + INDENT);
                    push_block(&mut lines, indent + INDENT, parts);
                }
                lines.push(format!("{}{broken_close}", spaces(indent)));
            }
        }
        match element.children.as_deref() {
            None | Some([]) => {}
            Some(children) => {
                for child in children {
                    let parts = self.child_lines(child, indent + INDENT, indent + INDENT);
                    push_block(&mut lines, indent + INDENT, parts);
                }
                lines.push(format!("{}</{}>", spaces(indent), element.tag));
            }
        }
        lines
    }

    fn attribute_lines(&self, attribute: &Attribute, indent: usize, column: usize) -> Vec<String> {
        let key = attribute_key(attribute);
        let column = column + key.chars().count() + 1;
        match &attribute.value {
            None => vec![key],
            Some(Value::Bare(range)) => {
                prefix(self.expr_lines(range, indent, column), &format!("{key}="))
            }
            Some(Value::Braced(range)) => prefix(
                brace(self.expr_lines(range, indent, column + 1)),
                &format!("{key}="),
            ),
        }
    }

    fn view_text(&self, roots: &[Child]) -> Option<String> {
        match roots {
            [] => Some("view! {}".to_owned()),
            [root] => Some(format!("view! {{ {} }}", self.child_text(root)?)),
            _ => None,
        }
    }

    fn child_text(&self, child: &Child) -> Option<String> {
        match child {
            Child::Element(element) => self.element_text(element),
            Child::Expr(range) => Some(format!("{{{}}}", self.expr_text(range)?)),
        }
    }

    fn element_text(&self, element: &Element) -> Option<String> {
        let head = self.attributes_text(element)?;
        match element.children.as_deref() {
            None => Some(format!("{head} />")),
            Some([]) => Some(format!("{head}></{}>", element.tag)),
            Some(_) => None,
        }
    }

    fn attributes_text(&self, element: &Element) -> Option<String> {
        let mut text = format!("<{}", element.tag);
        for attribute in &element.attributes {
            text.push(' ');
            text.push_str(&self.attribute_text(attribute)?);
        }
        Some(text)
    }

    fn attribute_text(&self, attribute: &Attribute) -> Option<String> {
        let key = attribute_key(attribute);
        match &attribute.value {
            None => Some(key),
            Some(Value::Bare(range)) => Some(format!("{key}={}", self.expr_text(range)?)),
            Some(Value::Braced(range)) => Some(format!("{key}={{{}}}", self.expr_text(range)?)),
        }
    }

    fn expr_text(&self, range: &Range<usize>) -> Option<String> {
        let mut rendered = String::new();
        let mut cursor = range.start;
        for nested in self.nested_views(range) {
            rendered.push_str(one_line(self.source, cursor..nested.range.start)?);
            rendered.push_str(&self.view_text(&nested.roots)?);
            cursor = nested.range.end;
        }
        rendered.push_str(one_line(self.source, cursor..range.end)?);
        Some(rendered)
    }

    fn expr_lines(&self, range: &Range<usize>, indent: usize, column: usize) -> Vec<String> {
        if let Some(single) = self.expr_text(range)
            && column + single.chars().count() <= MAX_WIDTH
        {
            return vec![single];
        }
        let shift = (!self.spans_lines_in_a_literal(range)).then(|| {
            (
                base_indent(text(self.source, range.clone()), indent),
                indent,
            )
        });
        let mut lines = Vec::new();
        let mut current = String::new();
        let mut cursor = range.start;
        for nested in self.nested_views(range) {
            self.push_source(&mut lines, &mut current, cursor..nested.range.start, shift);
            let (nested_indent, nested_column) = if lines.is_empty() {
                (indent, column + current.chars().count())
            } else {
                (leading(&current), current.chars().count())
            };
            let mut parts = self
                .view_lines(&nested.roots, nested_indent, nested_column)
                .into_iter();
            current.push_str(&parts.next().unwrap_or_default());
            for part in parts {
                lines.push(std::mem::replace(&mut current, part));
            }
            cursor = nested.range.end;
        }
        self.push_source(&mut lines, &mut current, cursor..range.end, shift);
        lines.push(current);
        lines
    }

    fn push_source(
        &self,
        lines: &mut Vec<String>,
        current: &mut String,
        range: Range<usize>,
        shift: Option<(usize, usize)>,
    ) {
        for (index, part) in text(self.source, range).split('\n').enumerate() {
            if index == 0 {
                current.push_str(part);
                continue;
            }
            let finished = std::mem::take(current);
            let Some((base, indent)) = shift else {
                lines.push(finished);
                current.push_str(part);
                continue;
            };
            lines.push(finished.trim_end().to_owned());
            let trimmed = part.trim_start();
            let original = part.len() - trimmed.len();
            current.push_str(&spaces(indent + original.saturating_sub(base)));
            current.push_str(trimmed);
        }
    }

    fn spans_lines_in_a_literal(&self, range: &Range<usize>) -> bool {
        let (from, to) = self.token_span(range);
        self.tokens[from..to]
            .iter()
            .any(|token| is_literal(token.kind) && token.text.contains('\n'))
    }

    fn token_span(&self, range: &Range<usize>) -> (usize, usize) {
        let from = self
            .tokens
            .partition_point(|token| token.range.start < range.start);
        let to = self
            .tokens
            .partition_point(|token| token.range.end <= range.end);
        (from, to)
    }

    fn nested_views(&self, range: &Range<usize>) -> Vec<ViewCall> {
        let (from, to) = self.token_span(range);
        scan_views(self.tokens, from, to)
    }
}

fn attribute_key(attribute: &Attribute) -> String {
    let marker = if attribute.special { "@" } else { "" };
    format!("{marker}{}", attribute.key)
}

fn one_line(source: &[u8], range: Range<usize>) -> Option<&str> {
    let slice = text(source, range);
    (!slice.contains('\n')).then_some(slice)
}

fn push_block(lines: &mut Vec<String>, indent: usize, parts: Vec<String>) {
    let mut parts = parts.into_iter();
    if let Some(first) = parts.next() {
        lines.push(format!("{}{first}", spaces(indent)));
    }
    lines.extend(parts);
}

fn brace(mut lines: Vec<String>) -> Vec<String> {
    lines[0].insert(0, '{');
    lines
        .last_mut()
        .expect("rendered lines are not empty")
        .push('}');
    lines
}

fn prefix(mut lines: Vec<String>, head: &str) -> Vec<String> {
    lines[0].insert_str(0, head);
    lines
}

fn base_indent(source: &str, fallback: usize) -> usize {
    source
        .split('\n')
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(leading)
        .min()
        .unwrap_or(fallback)
}

fn leading(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn spaces(count: usize) -> String {
    " ".repeat(count)
}
