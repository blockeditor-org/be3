use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use beui::Vec2;
use beui::unstyled::TextWidget;
use block_client::{
    BlockClient, block_ref::BlockRef, block_ref_url,
    blocks::version_control_worktree::VersionControlWorktreeMembership, parse_block_urls,
};
use block_editor_plugin::{
    EditorHost, Task,
    block_ui::{self, BlockLabel},
};
use text_editor_core::{EditorCommand, TextLanguage};
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
    pub reference: BlockRef,
    pub large: bool,
}

pub(crate) struct PendingEmbed {
    pub task: Task<BlockRef>,
    pub source_name: String,
    pub markdown: bool,
}

#[derive(Default)]
pub(crate) struct EmbedReferenceCache {
    resolved: HashMap<BlockRef, Option<Uuid>>,
    pending: Vec<(BlockRef, Task<Option<Uuid>>)>,
}

impl EmbedReferenceCache {
    fn poll(&mut self) {
        let mut finished = Vec::new();
        self.pending.retain_mut(|(reference, task)| {
            if !task.finished() {
                task.poll();
            }
            if !task.finished() {
                return true;
            }
            finished.push((*reference, task.take().flatten()));
            false
        });
        for (reference, resolved) in finished {
            self.resolved.insert(reference, resolved);
        }
    }

    fn resolve(
        &mut self,
        host: &EditorHost,
        client: &Arc<BlockClient>,
        referencing_id: Uuid,
        reference: BlockRef,
    ) -> Option<Uuid> {
        if let Some(id) = reference.as_direct() {
            return Some(id);
        }
        if let Some(resolved) = self.resolved.get(&reference) {
            return *resolved;
        }
        if !self
            .pending
            .iter()
            .any(|(pending, _)| *pending == reference)
        {
            let client = Arc::clone(client);
            let task = host.spawn(async move {
                client
                    .resolve_reference(
                        referencing_id,
                        &reference,
                        &VersionControlWorktreeMembership,
                    )
                    .await
            });
            self.pending.push((reference, task));
        }
        None
    }
}

pub(crate) fn poll_pending_embeds(state: &State) {
    let mut finished = Vec::new();
    state.pending_embeds.borrow_mut().retain_mut(|pending| {
        let Some(reference) = pending.task.take() else {
            return !pending.task.finished();
        };
        finished.push((reference, pending.source_name.clone(), pending.markdown));
        false
    });
    for (reference, source_name, markdown) in finished {
        let directive =
            image_embed_directive(state.workspace_id, &reference, &source_name, markdown);
        state
            .text
            .execute(EditorCommand::InsertText(directive.as_bytes()));
        state.text.reveal_cursor();
    }
}

pub(crate) fn resolve_embeds(state: &State) -> Vec<ResolvedEmbed> {
    state.references.borrow_mut().poll();
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
                    block_client::properties::read_name(&reference.properties),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let block_id = state.block_id;
    let host = state.host().clone();
    let types = host.block_types();
    parsed
        .into_iter()
        .filter_map(|embed| {
            let id = state
                .references
                .borrow_mut()
                .resolve(&host, &state.client, block_id, embed.reference)
                .unwrap_or(Uuid::nil());
            (id != block_id).then_some((embed, id))
        })
        .map(|(embed, id)| {
            let metadata = referenced.get(&id).cloned().or_else(|| {
                state.client.cached_block(id).map(|block| {
                    (
                        block.block_type,
                        block_client::properties::read_name(&block.properties),
                    )
                })
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
                |(block_type, name)| BlockLabel::new(types.as_ref(), *block_type, name.as_ref()),
            );
            ResolvedEmbed {
                range: embed.range,
                id,
                block_type: metadata
                    .as_ref()
                    .map_or_else(Uuid::nil, |(block_type, _)| *block_type),
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
    reference: &BlockRef,
    source_name: &str,
    markdown: bool,
) -> String {
    let url = block_ref_url(workspace_id, reference);
    if markdown {
        format!("![{source_name}]({url})")
    } else {
        url
    }
}

pub(crate) fn parse_embeds(bytes: &[u8], workspace_id: Uuid, markdown: bool) -> Vec<ParsedEmbed> {
    parse_block_urls(bytes)
        .into_iter()
        .filter(|url| {
            url.workspace_id
                .is_none_or(|url_workspace| url_workspace == workspace_id)
        })
        .map(|url| {
            let image_range = markdown
                .then(|| markdown_image_range(bytes, &url.range))
                .flatten();
            ParsedEmbed {
                range: url.range,
                reference: url.reference,
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
