use std::cell::RefCell;

use beui::NodeId;
use beui::reactive::{Prop, clone, component, create_memo, create_signal, view};
use beui::styled::Link;
use beui::styled::theme::FONT_BODY;
use block_client::BlockHandleAccess;
use block_ui::BlockLabel;

use crate::{ChildTarget, Editor};

#[derive(Clone, Default, PartialEq)]
struct Shown {
    name: String,
    glyph: String,
}

#[component]
pub fn BlockLink(
    editor: Editor,
    block: Prop<Option<ChildTarget>>,
    #[prop(default = String::from("Loading…"))] fallback: Prop<String>,
    #[prop(default = FONT_BODY)] font_size: Prop<f32>,
) -> NodeId {
    let target = create_memo(move || block.get());
    let (shown, set_shown) = create_signal(Shown::default());
    let opened = RefCell::new(None::<(ChildTarget, Box<dyn BlockHandleAccess>)>);
    let client = editor.client().clone();
    let host = editor.host().clone();
    let catalog = editor.host().clone();
    editor.each_frame(clone!(target -> move || {
        let Some(next) = target.get_untracked() else {
            opened.borrow_mut().take();
            set_shown.set(Shown::default());
            return;
        };
        let mut handle = opened.borrow_mut();
        if handle.as_ref().is_none_or(|(current, _)| *current != next) {
            *handle = block_client::blocks::open(&client, next.id, next.block_type)
                .map(|opened| (next, opened));
        }
        let types = catalog.block_types();
        let label = match handle.as_ref() {
            Some((_, opened)) => BlockLabel::for_handle(types.as_ref(), opened.as_ref()),
            None => BlockLabel::new(types.as_ref(), next.block_type, None),
        };
        set_shown.set(Shown {
            name: label.name,
            glyph: label.icon.map(|icon| icon.codepoint.to_owned()).unwrap_or_default(),
        });
    }));
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
