use std::collections::HashMap;
use std::ops::Range;

use beui::Vec2;
use beui::unstyled::TextWidget;
use block_editor_beui::be_block::block_url::{block_url, parse_block_urls};
use block_editor_beui::block_ui::{self, BlockLabel};
use text_editor_core::TextLanguage;
use uuid::Uuid;

use super::state::State;

pub(crate) const UNAVAILABLE_EMBED_SIZE: Vec2 = Vec2::new(320.0, 120.0);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedEmbed {
    pub range: Range<usize>,
    pub id: Uuid,
    pub block_type: Uuid,
    pub label: String,
    pub icon: Option<&'static str>,
    pub automatic: bool,
    pub large: bool,
    pub available: bool,
    pub frame_size: Option<Vec2>,
}

impl ResolvedEmbed {
    pub fn widget(&self) -> TextWidget {
        TextWidget {
            range: self.range.clone(),
            label: self.label.clone(),
            icon: self.icon,
            italic: self.automatic,
            broken: !self.available,
            block_size: self
                .large
                .then(|| self.frame_size.unwrap_or(UNAVAILABLE_EMBED_SIZE)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedEmbed {
    pub range: Range<usize>,
    pub reference: Uuid,
    pub large: bool,
}

pub(crate) fn resolve_embeds(state: &State) -> Vec<ResolvedEmbed> {
    let markdown = state.text.language() == TextLanguage::Markdown;
    let parsed = parse_embeds(&state.text.bytes(), state.workspace_id, markdown);
    let references = state.dependencies.read();
    let referenced = references
        .iter()
        .map(|reference| {
            (
                reference.id,
                (
                    reference.block_type,
                    reference.name.clone(),
                    reference.named_by_hand,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let block_id = state.block_id;
    let types = state.host().block_types();
    parsed
        .into_iter()
        .filter(|embed| embed.reference != block_id)
        .map(|embed| {
            let id = embed.reference;
            let metadata = referenced.get(&id).cloned().or_else(|| {
                state
                    .client
                    .info(id)
                    .map(|block| (block.block_type, block.name, block.named_by_hand))
            });
            let frame_size = embed
                .large
                .then(|| state.embed_sizes.borrow().get(&id).copied())
                .flatten()
                .map(|intrinsic| {
                    let (width, height) =
                        block_ui::embedded_editor_frame(intrinsic.x, intrinsic.y, 1.0);
                    Vec2::new(width, height)
                });
            let label = metadata.as_ref().map_or_else(
                || BlockLabel {
                    block_type: Uuid::nil(),
                    icon: None,
                    name: "Broken link".to_owned(),
                    automatic: true,
                },
                |(block_type, name, by_hand)| {
                    BlockLabel::new(types.as_ref(), *block_type, name.as_deref(), *by_hand)
                },
            );
            ResolvedEmbed {
                range: embed.range,
                id,
                block_type: metadata
                    .as_ref()
                    .map_or_else(Uuid::nil, |(block_type, _, _)| *block_type),
                label: label.name,
                icon: label.icon,
                automatic: label.automatic,
                large: embed.large,
                available: metadata.is_some(),
                frame_size,
            }
        })
        .collect()
}

pub(crate) fn image_embed_directive(
    workspace_id: Uuid,
    reference: &Uuid,
    source_name: &str,
    markdown: bool,
) -> String {
    let url = block_url(workspace_id, *reference);
    if markdown {
        format!("![{source_name}]({url})")
    } else {
        url
    }
}

pub(crate) fn parse_embeds(bytes: &[u8], workspace_id: Uuid, markdown: bool) -> Vec<ParsedEmbed> {
    parse_block_urls(bytes)
        .into_iter()
        .filter(|url| url.workspace_id == workspace_id)
        .map(|url| {
            let image_range = markdown
                .then(|| markdown_image_range(bytes, &url.range))
                .flatten();
            ParsedEmbed {
                range: url.range,
                reference: url.block,
                large: image_range.is_some(),
            }
        })
        .collect()
}

fn markdown_image_range(bytes: &[u8], url: &Range<usize>) -> Option<Range<usize>> {
    if url.start < 3
        || bytes.get(url.start - 2..url.start)? != b"]("
        || bytes.get(url.end) != Some(&b')')
    {
        return None;
    }
    let line_start = bytes[..url.start - 2]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |newline| newline + 1);
    let image_start = bytes[line_start..url.start - 2]
        .windows(2)
        .rposition(|window| window == b"![")?
        + line_start;
    let image_end = url.end + 1;
    let line_end = bytes[image_end..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |newline| image_end + newline);
    let whitespace = |byte: &u8| matches!(*byte, b' ' | b'\t' | b'\r');
    (bytes[line_start..image_start].iter().all(whitespace)
        && bytes[image_end..line_end].iter().all(whitespace))
    .then_some(image_start..image_end)
}
