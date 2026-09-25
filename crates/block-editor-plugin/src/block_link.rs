use std::cell::RefCell;

use beui::NodeId;
use beui::reactive::{Memo, Prop, ReadSignal, clone, component, create_memo, create_signal, view};
use beui::styled::Link;
use beui::styled::theme::FONT_BODY;
use block_ui::{BlockLabel, BlockTypes};

use crate::{BlockList, BlockQuery, ChildTarget, Editor};

#[derive(Clone, Default, PartialEq)]
pub struct BlockDisplay {
    pub name: String,
    pub glyph: String,
    pub type_name: String,
    pub automatic: bool,
    pub resolved: bool,
}

pub fn watch_block_label(
    editor: &Editor,
    target: Memo<Option<ChildTarget>>,
) -> ReadSignal<BlockDisplay> {
    let (shown, set_shown) = create_signal(BlockDisplay::default());
    let shown_now = shown.clone();
    let opened = RefCell::new(None::<(ChildTarget, BlockList)>);
    let blocks = editor.blocks();
    let catalog = editor.host().clone();
    editor.each_frame(move || {
        let Some(next) = target.get_untracked() else {
            opened.borrow_mut().take();
            set_shown.set(BlockDisplay::default());
            return;
        };
        let mut list = opened.borrow_mut();
        if list.as_ref().is_none_or(|(current, _)| *current != next) {
            *list = Some((next, blocks.watch(BlockQuery::Block(next.id))));
        }
        let info = list
            .as_ref()
            .and_then(|(_, list)| list.read().into_iter().next());
        let types = catalog.block_types();
        let resolved = info.is_some();
        let label = BlockLabel::new(
            types.as_ref(),
            next.block_type,
            info.as_ref().and_then(|info| info.name.as_deref()),
            info.as_ref().is_some_and(|info| info.named_by_hand),
        );
        let display = BlockDisplay {
            name: label.name,
            glyph: label.icon.map(str::to_owned).unwrap_or_default(),
            type_name: types
                .display_name(next.block_type)
                .map_or_else(|| next.block_type.to_string(), str::to_owned),
            automatic: label.automatic,
            resolved,
        };
        if shown_now.get_untracked() != display {
            set_shown.set(display);
        }
    });
    shown
}

#[component]
pub fn BlockLink(
    editor: Editor,
    block: Prop<Option<ChildTarget>>,
    #[prop(default = String::from("Loading…"))] fallback: Prop<String>,
    #[prop(default = FONT_BODY)] font_size: Prop<f32>,
) -> NodeId {
    let target = create_memo(move || block.get());
    let shown = watch_block_label(&editor, target.clone());
    let host = editor.host().clone();
    let label = create_memo(clone!(shown -> move || {
        let name = shown.with(|shown| shown.name.clone());
        match name.is_empty() {
            true => fallback.get(),
            false => name,
        }
    }));
    let glyph = create_memo(clone!(shown -> move || shown.with(|shown| shown.glyph.clone())));
    let missing = create_memo(clone!(target -> move || target.get().is_none()));
    let open = clone!(target -> move || {
        if let Some(target) = target.get_untracked() {
            host.open_block(target.id, target.block_type);
        }
    });
    view! {
        <Link
            label={label}
            glyph={glyph}
            disabled={missing}
            font_size={font_size}
            on_click={open}
        />
    }
}
