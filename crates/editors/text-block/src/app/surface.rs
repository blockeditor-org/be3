use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use block_editor_beui::{Drag, block_ui::BlockLabel};
use text_editor_core::{CursorLeftRightStop, EditorCommand};

use super::embeds::ResolvedEmbed;
use super::large_embed::{LargeEmbed, embed_is_live};
use super::state::{FocusedEmbed, State};

#[component]
pub(crate) fn TextSurface(state: State) -> NodeId {
    let embeds = state.embeds;
    let widgets = create_memo(move || {
        embeds
            .get()
            .iter()
            .map(ResolvedEmbed::widget)
            .collect::<Vec<TextWidget>>()
    });
    let cursors = state.text.cursors();
    let remote_cursors = create_memo(move || {
        cursors.get();
        let remote = state.remote_cursors.get();
        block_editor_beui::remote_carets(
            state.text,
            remote
                .into_iter()
                .map(|(_, cursor)| (cursor.caret, cursor.color)),
        )
    });
    let drag = state.editor.drag();
    let drop_caret = create_memo(move || drop_target(state, drag.get()));
    let embed_row = move |widget: usize| {
        let embed = state.embeds.get_untracked().get(widget).cloned();
        match embed {
            Some(embed) => view! {
                <LargeEmbed state={state} embed={embed} />
            },
            None => view! {
                <Frame />
            },
        }
    };
    let open_row = move |widget: usize| {
        view! {
            <OpenEmbed state widget />
        }
    };

    let dragged = state.editor.drag();
    create_effect(move || {
        let drag = dragged.get();
        untrack(|| take_drag(state, drag));
    });
    create_effect(move || {
        state.text.content().get();
        state.refresh_embeds();
    });
    let visible = state.editor.presence_visible();
    create_effect(move || {
        let visible = visible.get();
        state.text.cursors().get();
        untrack(|| state.publish_cursor(visible));
    });
    let revealed = state.editor.revealed();
    create_effect(move || {
        let Some(client_id) = revealed.get() else {
            return;
        };
        untrack(|| {
            if let Some(rect) = state.presence_cursor_rect(client_id) {
                state.text.reveal(rect);
            }
        });
    });

    let typing = state.typing;
    view! {
        <TextArea
            state={state.text}
            widgets={widgets}
            remote_cursors={remote_cursors}
            drop_caret={drop_caret}
            @test_id={"text.surface"}
            on_widget_press={move |widget: usize| focus_embed(state, widget)}
            on_focus_change={typing}
            pastes_images=true
            on_image_paste={move |image: PickedFile| paste_image(state, image)}
            block={embed_row}
            selected_widget={open_row}
        />
    }
}

#[component]
fn OpenEmbed(state: State, widget: usize) -> NodeId {
    let target = state
        .embeds
        .get_untracked()
        .get(widget)
        .map(|embed| (embed.id, embed.block_type));
    view! {
        <List spacing=Space::None align=Align::Start>
            <Frame padding={Padding::ZERO.top(Space::Sm)}>
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

fn focus_embed(state: State, widget: usize) -> bool {
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
        state.focused_embed.set(Some(key));
        state.inner().focus_confirmed.set(false);
    }
    true
}

fn drop_target(state: State, drag: Option<Drag>) -> Option<usize> {
    let drag = drag?;
    if drag.block_id == state.block_id {
        return None;
    }
    let byte = state.text.byte_at(drag.position)?;
    let position = state
        .text
        .with_core(|core| core.cursor_stop(byte, CursorLeftRightStop::UnicodeGraphemeCluster));
    state.text.with_core(|core| core.position_index(position))
}

fn take_drag(state: State, drag: Option<Drag>) {
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
    let position = state.text.with_core(|core| core.position(byte));
    state.text.execute(EditorCommand::SetSelection {
        anchor: position,
        focus: position,
    });
    let types = state.host().block_types();
    let name = match state.inner().client.info(drag.block_id) {
        Some(info) => info.label(types.as_ref()).name,
        None => BlockLabel::new(types.as_ref(), drag.block_type, None, false).name,
    };
    state.insert_image_embed(drag.block_id, &name);
    state.text.reveal_cursor();
}

fn paste_image(state: State, file: PickedFile) {
    let image = image_block_types::ImageFile::from_file(file.name, file.data);
    let source_name = image.header().source_name.clone();
    let id = state.create_image_block(&image);
    state.insert_image_embed(id, &source_name);
    state.import_error.set(None);
}
