use beui::reactive::{
    Frame, List, PickedFile, clone, component, create_effect, create_memo, untrack, view,
};
use beui::styled::{Button, ButtonVariant, TextArea};
use beui::unstyled::{RemoteTextCursor, TextWidget};
use beui::{NodeId, Vec2};
use block_editor_beui::{Drag, block_ui::BlockLabel};
use text_editor_core::{CursorLeftRightStop, CursorPosition, EditorCommand};

use super::embeds::ResolvedEmbed;
use super::large_embed::{LargeEmbed, embed_is_live};
use super::state::{FocusedEmbed, Shared};

const EMBED_BUTTON_GAP: f32 = 6.0;
const EMBED_BUTTON_SIZE: Vec2 = Vec2::new(72.0, 30.0);

#[component]
pub(crate) fn TextSurface(state: Shared) -> NodeId {
    let embeds = state.embeds;
    let widgets = create_memo(move || {
        embeds
            .get()
            .iter()
            .map(ResolvedEmbed::widget)
            .collect::<Vec<TextWidget>>()
    });
    let remote = state.presence_revision;
    let cursors = state.text.cursors();
    let remote_cursors = create_memo(clone!(state -> move || {
        remote.get();
        cursors.get();
        remote_cursors(&state)
    }));
    let drag = state.editor.drag();
    let drop_caret = create_memo(clone!(state -> move || drop_target(&state, drag.get())));
    let embed_row = clone!(state -> move |widget: usize| {
        let state = state.clone();
        let embed = state.embeds.get_untracked().get(widget).cloned();
        match embed {
            Some(embed) => view! {
                <LargeEmbed state={state} embed={embed} />
            },
            None => view! {
                <Frame />
            },
        }
    });
    let open_state = state.clone();
    let open_row = move |widget: usize| {
        let state = open_state.clone();
        view! {
            <OpenEmbed state widget />
        }
    };

    let dragged = state.editor.drag();
    create_effect(clone!(state -> move || {
        let drag = dragged.get();
        untrack(|| take_drag(&state, drag));
    }));
    create_effect(clone!(state -> move || {
        state.text.content().get();
        state.refresh_embeds();
    }));
    let visible = state.editor.presence_visible();
    create_effect(clone!(state -> move || {
        let visible = visible.get();
        state.peers.with(|_| ());
        state.text.cursors().get();
        untrack(|| state.poll_presence(visible));
    }));
    let revealed = state.editor.revealed();
    create_effect(clone!(state -> move || {
        let Some(client_id) = revealed.get() else {
            return;
        };
        untrack(|| {
            if let Some(rect) = state.presence_cursor_rect(client_id) {
                state.text.reveal(rect);
            }
        });
    }));

    let press_state = state.clone();
    let paste_state = state.clone();
    let typing = state.set_typing;
    view! {
        <TextArea
            state={state.text.clone()}
            widgets={widgets}
            remote_cursors={remote_cursors}
            drop_caret={drop_caret}
            @test_id={"text.surface"}
            on_widget_press={move |widget: usize| focus_embed(&press_state, widget)}
            on_focus_change={move |focused: bool| typing.set(focused)}
            pastes_images=true
            on_image_paste={move |image: PickedFile| paste_image(&paste_state, image)}
            block={embed_row}
            selected_widget={open_row}
        />
    }
}

#[component]
fn OpenEmbed(state: Shared, widget: usize) -> NodeId {
    let target = state
        .embeds
        .get_untracked()
        .get(widget)
        .map(|embed| (embed.id, embed.block_type));
    view! {
        <List spacing=0.0>
            <Frame height=EMBED_BUTTON_GAP />
            <Frame width={EMBED_BUTTON_SIZE.x} height={EMBED_BUTTON_SIZE.y}>
                <Button
                    label="Edit"
                    variant=ButtonVariant::Secondary
                    @test_id={"text.embed.open"}
                    on_click={move || {
                        if let Some((id, block_type)) = target {
                            state.host().open_block(id, block_type);
                        }
                    }}
                />
            </Frame>
        </List>
    }
}

fn focus_embed(state: &Shared, widget: usize) -> bool {
    let embeds = state.embeds.get_untracked();
    let Some(embed) = embeds.get(widget) else {
        return false;
    };
    if !embed.available {
        return false;
    }
    let key = FocusedEmbed {
        id: embed.id,
        source_start: embed.range.start,
    };
    if embed_is_live(state, key) {
        state.set_focused_embed.set(Some(key));
        state.focus_confirmed.set(false);
    }
    true
}

fn drop_target(state: &Shared, drag: Option<Drag>) -> Option<usize> {
    let drag = drag?;
    if drag.block_id == state.block_id {
        return None;
    }
    let byte = state.text.byte_at(drag.position)?;
    let position = state
        .text
        .core()
        .cursor_stop(byte, CursorLeftRightStop::UnicodeGraphemeCluster);
    state.text.core().position_index(position)
}

fn remote_cursors(state: &Shared) -> Vec<RemoteTextCursor> {
    let core = state.text.core();
    state
        .remote_cursors()
        .into_iter()
        .filter_map(|(_, cursor)| {
            let color = cursor.color;
            let selection =
                core.selection_range(&CursorPosition::range(cursor.anchor, cursor.focus))?;
            let caret = core.position_index(cursor.focus)?;
            Some(RemoteTextCursor {
                selection,
                caret,
                color: block_editor_beui::presence_color(color),
            })
        })
        .collect()
}

fn take_drag(state: &Shared, drag: Option<Drag>) {
    let Some(drag) = drag else {
        return;
    };
    if drag.block_id == state.block_id {
        return;
    }
    state.editor.accept_drag(true);
    if !drag.dropped {
        return;
    }
    let Some(byte) = drop_target(state, Some(drag)) else {
        return;
    };
    let position = state.text.core().position(byte);
    state.text.execute(EditorCommand::SetSelection {
        anchor: position,
        focus: position,
    });
    let types = state.host().block_types();
    let name = match state.client.info(drag.block_id) {
        Some(info) => info.label(types.as_ref()).name,
        None => BlockLabel::new(types.as_ref(), drag.block_type, None, false).name,
    };
    state.insert_image_embed(drag.block_id, &name);
    state.text.reveal_cursor();
}

fn paste_image(state: &Shared, file: PickedFile) {
    let image = block_editor_beui::be_block::ImageContent::from_file(file.name, file.data);
    let source_name = image.header().source_name.clone();
    let id = state.create_image_block(&image);
    state.insert_image_embed(id, &source_name);
    state.set_import_error.set(None);
}
