use std::ops::Range;
use tree_sitter_md::{MarkdownCursor, MarkdownTree};

use super::{SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize};
use crate::TextChange;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownTableAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTableRow {
    pub range: Range<usize>,
    pub cells: Vec<Range<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTable {
    pub rows: Vec<MarkdownTableRow>,
    pub alignments: Vec<MarkdownTableAlignment>,
}

pub(super) fn styles_in(tree: &MarkdownTree, window: Range<usize>) -> Vec<SynHlStyle> {
    let mut styles = vec![SynHlStyle::plain(SynHlColorScope::MarkdownPlainText); window.len()];
    let mut cursor = tree.walk();
    style_node(&mut cursor, &mut styles, window.start);
    styles
}

pub(super) fn tables(tree: &MarkdownTree) -> Vec<MarkdownTable> {
    let mut tables = Vec::new();
    let mut cursor = tree.walk();
    collect_tables(&mut cursor, &mut tables);
    tables
}

pub(super) fn code_blocks(tree: &MarkdownTree) -> Vec<Range<usize>> {
    let mut blocks = Vec::new();
    let mut cursor = tree.walk();
    collect_code_blocks(&mut cursor, &mut blocks);
    blocks
}

fn collect_code_blocks(cursor: &mut MarkdownCursor<'_>, blocks: &mut Vec<Range<usize>>) {
    let node = cursor.node();
    if matches!(node.kind(), "fenced_code_block" | "indented_code_block") {
        blocks.push(node.start_byte()..node.end_byte());
        return;
    }
    if cursor.goto_first_child() {
        loop {
            collect_code_blocks(cursor, blocks);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn collect_tables(cursor: &mut MarkdownCursor<'_>, tables: &mut Vec<MarkdownTable>) {
    if cursor.node().kind() == "pipe_table" {
        tables.push(table(cursor));
        return;
    }
    if cursor.goto_first_child() {
        loop {
            collect_tables(cursor, tables);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn table(cursor: &mut MarkdownCursor<'_>) -> MarkdownTable {
    let mut rows = Vec::new();
    let mut alignments = Vec::new();
    if cursor.goto_first_child() {
        loop {
            let kind = cursor.node().kind();
            if matches!(
                kind,
                "pipe_table_header" | "pipe_table_delimiter_row" | "pipe_table_row"
            ) {
                let delimiter = kind == "pipe_table_delimiter_row";
                let row = table_row(cursor, delimiter, &mut alignments);
                rows.push(row);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    MarkdownTable { rows, alignments }
}

fn table_row(
    cursor: &mut MarkdownCursor<'_>,
    delimiter: bool,
    alignments: &mut Vec<MarkdownTableAlignment>,
) -> MarkdownTableRow {
    let node = cursor.node();
    let range = node.start_byte()..node.end_byte();
    let mut cells = Vec::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if matches!(node.kind(), "pipe_table_cell" | "pipe_table_delimiter_cell") {
                cells.push(node.start_byte()..node.end_byte());
                if delimiter {
                    alignments.push(table_alignment(cursor));
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    MarkdownTableRow { range, cells }
}

fn table_alignment(cursor: &mut MarkdownCursor<'_>) -> MarkdownTableAlignment {
    let mut left = false;
    let mut right = false;
    if cursor.goto_first_child() {
        loop {
            match cursor.node().kind() {
                "pipe_table_align_left" => left = true,
                "pipe_table_align_right" => right = true,
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    match (left, right) {
        (true, true) => MarkdownTableAlignment::Center,
        (_, true) => MarkdownTableAlignment::Right,
        _ => MarkdownTableAlignment::Left,
    }
}

fn style_node(cursor: &mut MarkdownCursor<'_>, styles: &mut [SynHlStyle], offset: usize) {
    let node = cursor.node();
    let window_end = offset + styles.len();
    if node.end_byte() < offset || node.start_byte() >= window_end {
        return;
    }
    let range = node.start_byte().clamp(offset, window_end) - offset
        ..node.end_byte().clamp(offset, window_end) - offset;
    let kind = node.kind();
    let parent_kind = node.parent().map(|parent| parent.kind());

    let contextual_symbol = match kind {
        "[" | "]" => parent_kind.is_some_and(|parent| {
            matches!(
                parent,
                "image"
                    | "inline_link"
                    | "shortcut_link"
                    | "collapsed_reference_link"
                    | "full_reference_link"
            )
        }),
        "(" | ")" => parent_kind.is_some_and(|parent| matches!(parent, "image" | "inline_link")),
        "!" => parent_kind == Some("image"),
        "|" => parent_kind.is_some_and(|parent| parent.starts_with("pipe_table")),
        "~" => parent_kind == Some("strikethrough"),
        _ => false,
    };
    if contextual_symbol {
        for style in &mut styles[range.clone()] {
            set_symbol(style);
        }
    }

    match kind {
        "strong_emphasis" => {
            for style in &mut styles[range.clone()] {
                style.bold = true;
            }
        }
        "emphasis" => {
            for style in &mut styles[range.clone()] {
                style.italic = true;
            }
        }
        "strikethrough" => {
            for style in &mut styles[range.clone()] {
                style.strikethrough = true;
            }
        }
        "atx_heading" => {
            let level = node
                .child(0)
                .and_then(|marker| marker.kind().strip_prefix("atx_h"))
                .and_then(|value| value.strip_suffix("_marker"))
                .and_then(|value| value.parse().ok())
                .unwrap_or(6);
            set_heading(&mut styles[range.clone()], level);
        }
        "setext_heading" => {
            let mut child = node.walk();
            let level = node
                .children(&mut child)
                .find_map(|child| match child.kind() {
                    "setext_h1_underline" => Some(1),
                    "setext_h2_underline" => Some(2),
                    _ => None,
                })
                .unwrap_or(2);
            set_heading(&mut styles[range.clone()], level);
        }
        "block_quote" => {
            for style in &mut styles[range.clone()] {
                style.italic = true;
            }
        }
        "pipe_table_header" => {
            for style in &mut styles[range.clone()] {
                style.bold = true;
            }
        }
        "code_span" | "indented_code_block" | "code_fence_content" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownCode;
                style.family = SynHlFontFamily::Monospace;
                style.size = SynHlTextSize::Body;
                style.bold = false;
                style.italic = false;
            }
        }
        "info_string" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.family = SynHlFontFamily::Monospace;
            }
        }
        "link_text" | "link_label" | "image_description" | "uri_autolink" | "email_autolink" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.underline = true;
            }
        }
        "link_destination" | "link_title" => {
            for style in &mut styles[range.clone()] {
                style.color = SynHlColorScope::MarkdownLink;
                style.family = SynHlFontFamily::Monospace;
            }
        }
        "backslash_escape" => {
            if node.start_byte() >= offset
                && let Some(style) = styles.get_mut(range.start)
            {
                set_symbol(style);
            }
        }
        "emphasis_delimiter"
        | "code_span_delimiter"
        | "fenced_code_block_delimiter"
        | "block_quote_marker"
        | "block_continuation"
        | "list_marker_plus"
        | "list_marker_minus"
        | "list_marker_star"
        | "list_marker_dot"
        | "list_marker_parenthesis"
        | "task_list_marker_checked"
        | "task_list_marker_unchecked"
        | "thematic_break"
        | "setext_h1_underline"
        | "setext_h2_underline"
        | "pipe_table_delimiter_row"
        | "pipe_table_delimiter_cell"
        | "pipe_table_align_left"
        | "pipe_table_align_right"
        | "hard_line_break"
        | "atx_h1_marker"
        | "atx_h2_marker"
        | "atx_h3_marker"
        | "atx_h4_marker"
        | "atx_h5_marker"
        | "atx_h6_marker" => {
            for style in &mut styles[range.clone()] {
                set_symbol(style);
            }
        }
        _ => {}
    }

    if cursor.goto_first_child() {
        loop {
            style_node(cursor, styles, offset);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn set_heading(styles: &mut [SynHlStyle], level: u8) {
    for style in styles {
        style.bold = true;
        style.size = SynHlTextSize::Heading(level.clamp(1, 6));
    }
}

fn set_symbol(style: &mut SynHlStyle) {
    *style = SynHlStyle::plain(SynHlColorScope::MarkdownSymbol);
}

pub(super) fn collect_chain(
    cursor: &mut MarkdownCursor<'_>,
    start: usize,
    end: usize,
    result: &mut Vec<(usize, usize)>,
) {
    let node = cursor.node();
    if node.start_byte() > start || node.end_byte() < end {
        return;
    }
    result.push((node.start_byte(), node.end_byte()));
    if cursor.goto_first_child() {
        loop {
            collect_chain(cursor, start, end, result);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

pub(super) const WINDOW_TARGET: usize = 1024;
const WINDOW_REACH: usize = 4 * 1024;

pub(super) struct MarkdownWindow {
    pub(super) len: usize,
    pub(super) styles: Vec<SynHlStyle>,
    pub(super) tables: Vec<MarkdownTable>,
    pub(super) code_blocks: Vec<Range<usize>>,
}

pub(super) fn fences(bytes: &[u8]) -> Vec<Range<usize>> {
    let mut fences = Vec::new();
    let mut open: Option<(usize, u8, usize)> = None;
    let mut start = 0;
    while start < bytes.len() {
        let end = memchr::memchr(b'\n', &bytes[start..])
            .map_or(bytes.len(), |newline| start + newline + 1);
        let line = &bytes[start..end];
        let indent = line.iter().take_while(|byte| **byte == b' ').count();
        if indent <= 3 {
            let marker = line.get(indent).copied();
            let run = line[indent..]
                .iter()
                .take_while(|byte| Some(**byte) == marker)
                .count();
            if matches!(marker, Some(b'`' | b'~')) && run >= 3 {
                let marker = marker.unwrap_or(b'`');
                match open {
                    None => open = Some((start, marker, run)),
                    Some((opened, wanted, length)) if wanted == marker && run >= length => {
                        if line[indent + run..].iter().all(u8::is_ascii_whitespace) {
                            fences.push(opened..end);
                            open = None;
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        start = end;
    }
    if let Some((opened, _, _)) = open {
        fences.push(opened..bytes.len());
    }
    fences
}

fn fence_at(fences: &[Range<usize>], index: usize) -> Option<&Range<usize>> {
    let found = fences.partition_point(|fence| fence.end <= index);
    fences.get(found).filter(|fence| fence.start <= index)
}

fn is_blank_line_start(bytes: &[u8], index: usize) -> bool {
    (index == 0 || bytes[index - 1] == b'\n')
        && bytes[index..]
            .iter()
            .take_while(|byte| **byte != b'\n')
            .all(u8::is_ascii_whitespace)
}

fn line_start_before(bytes: &[u8], index: usize) -> usize {
    memchr::memrchr(b'\n', &bytes[..index]).map_or(0, |newline| newline + 1)
}

fn line_end_after(bytes: &[u8], index: usize) -> usize {
    memchr::memchr(b'\n', &bytes[index.min(bytes.len())..])
        .map_or(bytes.len(), |newline| index + newline + 1)
}

pub(super) fn touched_lines(bytes: &[u8], start: usize, end: usize) -> Range<usize> {
    line_start_before(bytes, start)..line_end_after(bytes, end)
}

fn is_fence_line(line: &[u8]) -> bool {
    let indent = line.iter().take_while(|byte| **byte == b' ').count();
    let Some(marker @ (b'`' | b'~')) = line.get(indent).copied() else {
        return false;
    };
    indent <= 3
        && line[indent..]
            .iter()
            .take_while(|byte| **byte == marker)
            .count()
            >= 3
}

fn has_fence_line(bytes: &[u8]) -> bool {
    bytes.split(|byte| *byte == b'\n').any(is_fence_line)
}

pub(super) fn shifted_fences(
    old_bytes: &[u8],
    old_fences: &[Range<usize>],
    bytes: &[u8],
    change: TextChange,
) -> Vec<Range<usize>> {
    let old_lines = touched_lines(old_bytes, change.start, change.old_end);
    let new_lines = touched_lines(bytes, change.start, change.new_end);
    if has_fence_line(&old_bytes[old_lines]) || has_fence_line(&bytes[new_lines]) {
        return fences(bytes);
    }
    let shifted: Option<Vec<Range<usize>>> = old_fences
        .iter()
        .map(|fence| Some(change.moved(fence.start)?..change.moved(fence.end)?))
        .collect();
    shifted.unwrap_or_else(|| fences(bytes))
}

pub(super) fn shifted_table_starts(
    old_bytes: &[u8],
    old_starts: &[usize],
    bytes: &[u8],
    change: TextChange,
) -> Vec<usize> {
    let old_lines = touched_lines(old_bytes, change.start, change.old_end);
    let from = line_start_before(old_bytes, old_lines.start.saturating_sub(1));
    let new_lines = touched_lines(bytes, change.start, change.new_end);
    let scanned_end = line_end_after(bytes, new_lines.end);
    let delta = |at: usize| at - old_lines.end + new_lines.end;
    let mut starts: Vec<usize> = old_starts
        .iter()
        .copied()
        .take_while(|start| *start < from)
        .collect();
    starts.extend(
        table_starts(&bytes[from..scanned_end])
            .into_iter()
            .map(|start| start + from)
            .filter(|start| *start < new_lines.end),
    );
    starts.extend(
        old_starts
            .iter()
            .copied()
            .filter(|start| *start >= old_lines.end)
            .map(delta),
    );
    starts
}

fn boundary_before(bytes: &[u8], fences: &[Range<usize>], from: usize) -> usize {
    let mut at = line_start_before(bytes, from);
    let limit = from.saturating_sub(WINDOW_REACH);
    loop {
        if let Some(fence) = fence_at(fences, at) {
            at = fence.start;
        }
        if at == 0 || (is_blank_line_start(bytes, at) && fence_at(fences, at).is_none()) {
            return at;
        }
        if at <= limit {
            let cut = line_start_before(bytes, from);
            return fence_at(fences, cut).map_or(cut, |fence| fence.start);
        }
        at = line_start_before(bytes, at - 1);
    }
}

fn boundary_after(bytes: &[u8], fences: &[Range<usize>], from: usize) -> usize {
    let mut at = from.min(bytes.len());
    at = bytes[at..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |newline| at + newline + 1);
    let limit = from.saturating_add(WINDOW_REACH);
    loop {
        if let Some(fence) = fence_at(fences, at) {
            at = fence.end;
        }
        if at >= bytes.len() || (is_blank_line_start(bytes, at) && fence_at(fences, at).is_none()) {
            return at.min(bytes.len());
        }
        if at >= limit {
            return at;
        }
        at = bytes[at..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |newline| at + newline + 1);
    }
}

pub(super) fn window_range(
    bytes: &[u8],
    fences: &[Range<usize>],
    around: Range<usize>,
) -> Range<usize> {
    let start = boundary_before(
        bytes,
        fences,
        around.start.saturating_sub(WINDOW_TARGET / 2),
    );
    let end = boundary_after(bytes, fences, around.end.saturating_add(WINDOW_TARGET / 2));
    start..end.max(around.end.min(bytes.len()))
}

fn parse(bytes: &[u8]) -> Option<MarkdownTree> {
    tree_sitter_md::MarkdownParser::default().parse(bytes, None)
}

pub(super) fn parse_window(source: &[u8]) -> MarkdownWindow {
    let Some(tree) = parse(source) else {
        return MarkdownWindow {
            styles: vec![SynHlStyle::plain(SynHlColorScope::MarkdownPlainText); source.len()],
            len: source.len(),
            tables: Vec::new(),
            code_blocks: Vec::new(),
        };
    };
    MarkdownWindow {
        len: source.len(),
        styles: styles_in(&tree, 0..source.len()),
        tables: tables(&tree),
        code_blocks: code_blocks(&tree),
    }
}

pub(super) fn shift_table(table: &MarkdownTable, by: usize) -> MarkdownTable {
    let shift = |inner: &Range<usize>| inner.start + by..inner.end + by;
    MarkdownTable {
        rows: table
            .rows
            .iter()
            .map(|row| MarkdownTableRow {
                range: shift(&row.range),
                cells: row.cells.iter().map(shift).collect(),
            })
            .collect(),
        alignments: table.alignments.clone(),
    }
}

pub(super) fn table_starts(bytes: &[u8]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut previous: Option<(usize, bool)> = None;
    let mut start = 0;
    while start < bytes.len() {
        let end =
            memchr::memchr(b'\n', &bytes[start..]).map_or(bytes.len(), |newline| start + newline);
        let line = &bytes[start..end];
        if is_delimiter_row(line)
            && let Some((header, true)) = previous
        {
            starts.push(header);
        }
        previous = Some((start, memchr::memchr(b'|', line).is_some()));
        start = end + 1;
    }
    starts
}

fn is_delimiter_row(line: &[u8]) -> bool {
    let line = line.trim_ascii();
    memchr::memchr(b'|', line).is_some()
        && line.contains(&b'-')
        && line
            .iter()
            .all(|byte| matches!(byte, b'|' | b'-' | b':' | b' ' | b'\t'))
}

pub(super) fn chain(
    bytes: &[u8],
    fences: &[Range<usize>],
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let range = window_range(bytes, fences, start..end);
    let Some(tree) = parse(&bytes[range.clone()]) else {
        return Vec::new();
    };
    let document_len = range.len();
    let local_start = (start - range.start).min(document_len);
    let local_end = if start == end {
        (local_start + 1).min(document_len)
    } else {
        (end - range.start).min(document_len)
    };
    let mut result = Vec::new();
    let mut cursor = tree.walk();
    collect_chain(&mut cursor, local_start, local_end, &mut result);
    result.reverse();
    result.dedup();
    let mut chain: Vec<(usize, usize)> = result
        .into_iter()
        .map(|(from, to)| (from + range.start, to + range.start))
        .collect();
    if chain.last() != Some(&(0, bytes.len())) {
        chain.push((0, bytes.len()));
    }
    chain
}
