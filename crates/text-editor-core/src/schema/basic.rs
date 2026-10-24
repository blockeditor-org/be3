use std::ops::Range;

use super::{Break, Line, LineLook, ListMarker, Schema, Selected, Tool};
use crate::Language;
use crate::core::EditorCommand;
use crate::highlighter::{SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize};
use crate::rich::{Expand, Formats, MarkSet, Name, Scope, Seed, Value};

pub const BOLD: &str = "bold";
pub const ITALIC: &str = "italic";
pub const UNDERLINE: &str = "underline";
pub const STRIKETHROUGH: &str = "strikethrough";
pub const CODE: &str = "code";
pub const LINK: &str = "link";
pub const MENTION: &str = "mention";

pub const HEADING: &str = "heading";
pub const LIST: &str = "list";
pub const CHECKED: &str = "checked";
pub const INDENT: &str = "indent";
pub const QUOTE: &str = "quote";
pub const CODE_BLOCK: &str = "code_block";

pub const BULLET: &str = "bullet";
pub const NUMBER: &str = "number";
pub const CHECK: &str = "check";

const MAX_INDENT: i64 = 4;

pub struct Basic;

pub fn mark(range: Range<usize>, key: &str) -> Seed {
    mark_value(range, key, Value::Bool(true))
}

pub fn mark_value(range: Range<usize>, key: &str, value: Value) -> Seed {
    Seed {
        range,
        scope: Scope::Inline,
        expand: Basic.expand(key),
        key: Name::new(key),
        value,
    }
}

pub fn block(range: Range<usize>, key: &str, value: Value) -> Seed {
    Seed {
        range,
        scope: Scope::Block,
        expand: Expand::After,
        key: Name::new(key),
        value,
    }
}

pub fn depth(props: &MarkSet) -> i64 {
    props.get(INDENT).and_then(Value::as_int).unwrap_or(0)
}

pub fn line_looks(formats: &Formats) -> Vec<LineLook> {
    let mut counts: Vec<(i64, Option<Value>, usize)> = Vec::new();
    let mut looks: Vec<LineLook> = formats
        .blocks
        .iter()
        .map(|block| {
            let props = &block.props;
            let level = depth(props);
            let kind = props.get(LIST).cloned();
            match &kind {
                None => counts.clear(),
                Some(_) => {
                    counts.retain(|(held, _, _)| *held <= level);
                    match counts.last_mut() {
                        Some((held, same, count)) if *held == level && *same == kind => *count += 1,
                        Some((held, _, _)) if *held == level => {
                            counts.pop();
                            counts.push((level, kind.clone(), 1));
                        }
                        _ => counts.push((level, kind.clone(), 1)),
                    }
                }
            }
            let list = kind
                .as_ref()
                .and_then(Value::as_text)
                .and_then(|kind| match kind {
                    BULLET => Some(ListMarker::Bullet),
                    NUMBER => Some(ListMarker::Number(
                        counts.last().map_or(1, |(_, _, count)| *count),
                    )),
                    CHECK => Some(ListMarker::Check(on(props, CHECKED))),
                    _ => None,
                });
            LineLook {
                marker: block.start..block.start,
                list,
                depth: match list {
                    Some(_) => level.max(0) as usize,
                    None => 0,
                },
                continues: false,
                quote: usize::from(on(props, QUOTE)),
                code_block: props.get(CODE_BLOCK).is_some(),
                heading: match props.get(CODE_BLOCK) {
                    Some(_) => 0,
                    None => props
                        .get(HEADING)
                        .and_then(Value::as_int)
                        .filter(|level| (1..=6).contains(level))
                        .map_or(0, |level| level as u8),
                },
                ..LineLook::at(block.start)
            }
        })
        .collect();
    let code: Vec<Option<&Value>> = formats
        .blocks
        .iter()
        .map(|block| block.props.get(CODE_BLOCK))
        .collect();
    super::join_neighbours(&mut looks, &code);
    looks
}

fn on(selected: &MarkSet, key: &str) -> bool {
    selected.get(key).is_some_and(Value::is_true)
}

fn toggled(selected: &MarkSet, key: &str) -> Option<Value> {
    (!on(selected, key)).then_some(Value::Bool(true))
}

fn set_mark(key: &str, value: Option<Value>) -> EditorCommand<'static> {
    EditorCommand::SetMark(Name::new(key), value)
}

fn set_block(key: &str, value: Option<Value>) -> EditorCommand<'static> {
    EditorCommand::SetBlock {
        at: None,
        key: Name::new(key),
        value,
    }
}

fn heading_is(selected: &Selected, level: i64) -> bool {
    selected.block.get(HEADING).and_then(Value::as_int) == Some(level)
}

fn list_is(selected: &Selected, kind: &str) -> bool {
    selected.block.get(LIST).and_then(Value::as_text) == Some(kind)
}

fn toggle_list(selected: &Selected, kind: &str) -> EditorCommand<'static> {
    set_block(LIST, (!list_is(selected, kind)).then(|| Value::text(kind)))
}

macro_rules! inline_tool {
    ($name:expr, $label:expr, $shortcut:expr) => {
        Tool {
            name: $name,
            label: $label,
            shortcut: $shortcut,
            active: |selected| on(&selected.marks, $name),
            command: |selected| set_mark($name, toggled(&selected.marks, $name)),
        }
    };
}

macro_rules! heading_tool {
    ($name:expr, $label:expr, $level:expr) => {
        Tool {
            name: $name,
            label: $label,
            shortcut: None,
            active: |selected| heading_is(selected, $level),
            command: |selected| {
                set_block(
                    HEADING,
                    (!heading_is(selected, $level)).then_some(Value::Int($level)),
                )
            },
        }
    };
}

const TOOLS: [Tool; 12] = [
    inline_tool!(BOLD, "Bold", Some('b')),
    inline_tool!(ITALIC, "Italic", Some('i')),
    inline_tool!(UNDERLINE, "Underline", Some('u')),
    inline_tool!(STRIKETHROUGH, "Strikethrough", None),
    inline_tool!(CODE, "Code", Some('e')),
    heading_tool!("heading1", "Heading 1", 1),
    heading_tool!("heading2", "Heading 2", 2),
    Tool {
        name: BULLET,
        label: "Bulleted list",
        shortcut: None,
        active: |selected| list_is(selected, BULLET),
        command: |selected| toggle_list(selected, BULLET),
    },
    Tool {
        name: NUMBER,
        label: "Numbered list",
        shortcut: None,
        active: |selected| list_is(selected, NUMBER),
        command: |selected| toggle_list(selected, NUMBER),
    },
    Tool {
        name: CHECK,
        label: "Checklist",
        shortcut: None,
        active: |selected| list_is(selected, CHECK),
        command: |selected| toggle_list(selected, CHECK),
    },
    Tool {
        name: QUOTE,
        label: "Quote",
        shortcut: None,
        active: |selected| on(&selected.block, QUOTE),
        command: |selected| set_block(QUOTE, toggled(&selected.block, QUOTE)),
    },
    Tool {
        name: CODE_BLOCK,
        label: "Code block",
        shortcut: None,
        active: |selected| selected.block.get(CODE_BLOCK).is_some(),
        command: |selected| {
            set_block(
                CODE_BLOCK,
                selected
                    .block
                    .get(CODE_BLOCK)
                    .is_none()
                    .then(|| Value::text("rust")),
            )
        },
    },
];

impl Schema for Basic {
    fn expand(&self, key: &str) -> Expand {
        match key {
            LINK | MENTION => Expand::Never,
            _ => Expand::After,
        }
    }

    fn leaf_style(
        &self,
        marks: &MarkSet,
        block: &MarkSet,
        decoration: Option<SynHlStyle>,
    ) -> SynHlStyle {
        let code_block = block.get(CODE_BLOCK).is_some();
        let code = on(marks, CODE);
        let link = marks.get(LINK).is_some();
        let checked = on(block, CHECKED) && block.get(LIST).and_then(Value::as_text) == Some(CHECK);
        let heading = block
            .get(HEADING)
            .and_then(Value::as_int)
            .filter(|level| (1..=6).contains(level));
        let color = match decoration {
            Some(decoration) if code_block => decoration.color,
            _ if code => SynHlColorScope::MarkdownCode,
            _ if link => SynHlColorScope::MarkdownLink,
            _ if checked => SynHlColorScope::MarkdownSymbol,
            _ => SynHlColorScope::MarkdownPlainText,
        };
        SynHlStyle {
            color,
            family: match code || code_block {
                true => SynHlFontFamily::Monospace,
                false => SynHlFontFamily::Proportional,
            },
            size: match heading {
                Some(level) if !code_block => SynHlTextSize::Heading(level as u8),
                _ => SynHlTextSize::Body,
            },
            bold: on(marks, BOLD) || (heading.is_some() && !code_block),
            italic: on(marks, ITALIC),
            underline: on(marks, UNDERLINE) || link,
            strikethrough: on(marks, STRIKETHROUGH) || checked,
            code: code && !code_block,
        }
    }

    fn on_break(&self, line: &Line<'_>) -> Break {
        let Line {
            props: block,
            empty,
            at_end,
            above,
            below,
        } = *line;
        let code = block.get(CODE_BLOCK);
        let blank_above =
            above.is_some_and(|(above, empty)| empty && above.get(CODE_BLOCK) == code);
        let ends_code = below.is_none_or(|below| below.get(CODE_BLOCK) != code);
        if code.is_some() && empty && blank_above && ends_code {
            return Break::Exit(vec![Name::new(CODE_BLOCK)]);
        }
        if empty && block.get(LIST).is_some() && depth(block) > 0 {
            return Break::Lift(vec![(
                Name::new(INDENT),
                (depth(block) > 1).then(|| Value::Int(depth(block) - 1)),
            )]);
        }
        if empty && (block.get(LIST).is_some() || on(block, QUOTE)) {
            return Break::Lift(
                [LIST, CHECKED, INDENT, QUOTE]
                    .into_iter()
                    .map(|key| (Name::new(key), None))
                    .collect(),
            );
        }
        let mut reset = vec![Name::new(CHECKED)];
        if at_end {
            reset.push(Name::new(HEADING));
        }
        Break::Split { reset }
    }

    fn lift_on_backspace(&self, block: &MarkSet, previous: Option<&MarkSet>) -> Vec<Name> {
        let opens_code = block.get(CODE_BLOCK).is_some()
            && previous.and_then(|previous| previous.get(CODE_BLOCK)) != block.get(CODE_BLOCK);
        [HEADING, LIST, CHECKED, INDENT, QUOTE]
            .into_iter()
            .filter(|key| block.get(key).is_some())
            .chain(opens_code.then_some(CODE_BLOCK))
            .map(Name::new)
            .collect()
    }

    fn indent(&self, block: &MarkSet, outdent: bool) -> Option<(Name, Option<Value>)> {
        block.get(LIST)?;
        let depth = block.get(INDENT).and_then(Value::as_int).unwrap_or(0);
        let depth = match outdent {
            true => depth - 1,
            false => depth + 1,
        }
        .clamp(0, MAX_INDENT);
        Some((Name::new(INDENT), (depth > 0).then_some(Value::Int(depth))))
    }

    fn decorate(&self, block: &MarkSet) -> Option<Language> {
        match block.get(CODE_BLOCK)?.as_text()? {
            "rust" => Some(Language::Rust),
            "toml" => Some(Language::Toml),
            "zig" => Some(Language::Zig),
            _ => None,
        }
    }

    fn tools(&self) -> &[Tool] {
        &TOOLS
    }

    fn line_looks(&self, formats: &Formats) -> Vec<LineLook> {
        line_looks(formats)
    }

    fn check(&self, checked: bool) -> Option<(Name, Option<Value>)> {
        Some((Name::new(CHECKED), checked.then_some(Value::Bool(true))))
    }
}
