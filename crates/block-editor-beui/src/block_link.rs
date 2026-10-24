use beui::icons::Glyph;
use std::cell::RefCell;

use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use block_ui::{BlockLabel, BlockTypes, Reach};

use crate::{BlockList, BlockQuery, ChildTarget, Editor};

#[derive(Clone, Default, PartialEq)]
pub struct BlockDisplay {
    pub name: String,
    pub glyph: Option<Glyph>,
    pub type_name: String,
    pub automatic: bool,
    pub resolved: bool,
    pub reach: Reach,
    pub deleted: bool,
}

pub fn label_glyph(label: &BlockLabel) -> Option<Glyph> {
    match label.reach {
        Reach::Open if label.deleted => Some(beui::icons::ICON_TRASH),
        Reach::Open => label.icon,
        Reach::Locked | Reach::NoAccess => Some(beui::icons::ICON_LOCK),
    }
}

pub fn watch_block_label(
    editor: Editor,
    target: Memo<Option<ChildTarget>>,
) -> ReadSignal<BlockDisplay> {
    let shown = create_signal(BlockDisplay::default());
    let opened = RefCell::new(None::<(ChildTarget, BlockList)>);
    let blocks = editor.blocks();
    let catalog = editor.host();
    create_binding(move || {
        let Some(next) = target.get() else {
            opened.borrow_mut().take();
            shown.set(BlockDisplay::default());
            return;
        };
        let mut list = opened.borrow_mut();
        if list.as_ref().is_none_or(|(current, _)| *current != next) {
            *list = Some((next, blocks.watch(BlockQuery::Block(next.id))));
        }
        let info = list
            .as_ref()
            .and_then(|(_, list)| list.read().ok()?.into_iter().next());
        drop(list);
        let types = catalog.block_types();
        let resolved = info.is_some();
        let label = match &info {
            Some(info) => info.label(types.as_ref()),
            None => BlockLabel::new(types.as_ref(), next.block_type, None, false),
        };
        let display = BlockDisplay {
            glyph: label_glyph(&label),
            reach: label.reach,
            deleted: label.deleted,
            name: label.name,
            type_name: types
                .display_name(next.block_type)
                .map_or_else(|| next.block_type.to_string(), str::to_owned),
            automatic: label.automatic,
            resolved,
        };
        if shown.get_untracked() != display {
            shown.set(display);
        }
    });
    shown.read_only()
}

#[component]
pub fn BlockLink(
    editor: Editor,
    block: Prop<Option<ChildTarget>>,
    #[prop(default = String::from("Loading…"))] fallback: Prop<String>,
    #[prop(default = TextSize::Base)] size: Prop<TextSize>,
) -> NodeId {
    let target = create_memo(move || block.get());
    let shown = watch_block_label(editor, target);
    let host = editor.host();
    let label = create_memo(move || {
        let name = shown.with(|shown| shown.name.clone());
        match name.is_empty() {
            true => fallback.get(),
            false => name,
        }
    });
    let glyph = create_memo(move || shown.with(|shown| shown.glyph));
    let missing = create_memo(move || target.get().is_none());
    let open = move || {
        if let Some(target) = target.get_untracked() {
            host.open_block(target.id, target.block_type);
        }
    };
    view! {
        <Link label={label} glyph={glyph} disabled={missing} size on_click={open} />
    }
}
