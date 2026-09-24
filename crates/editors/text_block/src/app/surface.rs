use beui::reactive::{
    CanvasItem, ForEach, ReadSignal, Show, clone, component, create_memo, each_frame,
    request_paste, view,
};
use beui::styled::{Button, ButtonVariant, TextArea};
use beui::unstyled::{RemoteTextCursor, TextAreaLayout, TextWidget};
use beui::{Key, KeyPress, NodeId, Rect, Vec2};
use block_editor_plugin::{Drag, block_ui::BlockLabel};
use text_editor_core::{CursorLeftRightStop, CursorPosition, EditorCommand};

use super::embeds::ResolvedEmbed;
use super::large_embed::{LargeEmbed, embed_is_live};
use super::state::{FocusedEmbed, Shared};

const EMBED_BUTTON_GAP: f32 = 6.0;
const EMBED_BUTTON_SIZE: Vec2 = Vec2::new(72.0, 30.0);

#[component]
pub(crate) fn TextSurface(state: Shared) -> NodeId {
    let embeds = state.embeds.clone();
    let widgets = create_memo(clone!(embeds -> move || {
        embeds.get().iter().map(ResolvedEmbed::widget).collect::<Vec<TextWidget>>()
    }));
    let layout = state.text.layout();
    let remote = state.presence_revision.clone();
    let cursors = state.text.cursors();
    let remote_cursors = create_memo(clone!(state remote cursors -> move || {
        remote.get();
        cursors.get();
        remote_cursors(&state)
    }));
    let drag = state.editor.drag();
    let drop_caret = create_memo(clone!(state drag -> move || drop_target(&state, drag.get())));

    let block_widgets = create_memo(clone!(layout -> move || layout.get().block_widgets()));
    let embed_row = clone!(state layout -> move |widget: usize| {
        let state = state.clone();
        let layout = layout.clone();
        view! {
            <EmbedFrame state={state} layout={layout} widget={widget} />
        }
    });

    let selected_embed = create_memo(clone!(state layout cursors -> move || {
        cursors.get();
        let layout = layout.get();
        let ranges = state.text.selection_ranges();
        let selection = (ranges.len() == 1).then(|| ranges[0].clone())?;
        let embeds = state.embeds.get();
        let index = embeds
            .iter()
            .position(|embed| !embed.large && embed.range == selection)?;
        let rect = layout.widget_rect(index)?;
        Some((embeds[index].id, embeds[index].block_type, rect))
    }));
    let open_embed = create_memo(clone!(selected_embed -> move || selected_embed.get().is_some()));
    let open_state = state.clone();
    let open_row = move || {
        let target = selected_embed.clone();
        let state = open_state.clone();
        let rect = target
            .get_untracked()
            .map(|(_, _, rect)| rect)
            .unwrap_or(Rect::ZERO);
        view! {
            <CanvasItem
                x={rect.min.x}
                y={rect.max.y + EMBED_BUTTON_GAP}
                width={EMBED_BUTTON_SIZE.x}
                height={EMBED_BUTTON_SIZE.y}
            >
                <Button
                    label="Edit"
                    variant=ButtonVariant::Secondary
                    @test_id={"text.embed.open"}
                    on_click={move || {
                        if let Some((id, block_type, _)) = target.get_untracked() {
                            state.host().open_block(id, block_type);
                        }
                    }}
                />
            </CanvasItem>
        }
    };

    let frame_state = state.clone();
    each_frame(move || {
        poll_paste(&frame_state);
        poll_drag(&frame_state);
        frame_state.poll_external_edit();
        frame_state.refresh_embeds();
        frame_state.poll_presence(frame_state.editor.presence_visible().get_untracked());
        if let Some(client_id) = frame_state.editor.revealed().get_untracked()
            && let Some(rect) = frame_state.presence_cursor_rect(client_id)
        {
            frame_state.text.reveal(rect);
        }
    });

    let press_state = state.clone();
    let key_state = state.clone();
    view! {
        <TextArea
            state={state.text.clone()}
            widgets={widgets}
            remote_cursors={remote_cursors}
            drop_caret={drop_caret}
            @test_id={"text.surface"}
            on_widget_press={move |widget: usize| focus_embed(&press_state, widget)}
            on_key_override={move |press: KeyPress| paste_key(&key_state, press)}
        >
            <ForEach keys={block_widgets} view={embed_row} />
            <Show condition={open_embed} then={open_row} />
        </TextArea>
    }
}

#[component]
fn EmbedFrame(state: Shared, layout: ReadSignal<TextAreaLayout>, widget: usize) -> CanvasItem {
    let rect = create_memo(clone!(layout -> move || {
        layout.get().widget_rect(widget).unwrap_or(Rect::ZERO)
    }));
    let x = create_memo(clone!(rect -> move || rect.get().min.x));
    let y = create_memo(clone!(rect -> move || rect.get().min.y));
    let width = create_memo(clone!(rect -> move || rect.get().width()));
    let height = create_memo(clone!(rect -> move || rect.get().height()));
    let embed = state.embeds.get_untracked().get(widget).cloned();
    let Some(embed) = embed else {
        return view! {
            <CanvasItem x=0.0 y=0.0 width=0.0 height=0.0 />
        };
    };
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <LargeEmbed state={state} embed={embed} />
        </CanvasItem>
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

fn paste_key(state: &Shared, press: KeyPress) -> bool {
    if press.pressed && press.modifiers.ctrl && press.key == Key::V {
        state.paste_requested.set(true);
        return true;
    }
    false
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
                color: block_editor_plugin::block_ui::presence_color(color),
            })
        })
        .collect()
}

fn poll_drag(state: &Shared) {
    let Some(drag) = state.editor.drag().get_untracked() else {
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
    let name = match state.client.cached_block(drag.block_id) {
        Some(cached) => BlockLabel::for_cached(types.as_ref(), &cached).name,
        None => BlockLabel::new(types.as_ref(), drag.block_type, None).name,
    };
    state.insert_image_embed(drag.block_id, &name);
    state.text.reveal_cursor();
}

fn poll_paste(state: &Shared) {
    let asked = state.paste_requested.take();
    let pasted = state.paster.borrow_mut().paste(state.host(), asked);
    let Some(pasted) = pasted else {
        return;
    };
    match pasted {
        block_editor_plugin::PastedImage::Image { name, data } => {
            let image = block_editor_plugin::be_block::ImageContent::from_file(name, data);
            let source_name = image.header().source_name.clone();
            let id = state.create_image_block(&image);
            state.insert_image_embed(id, &source_name);
            state.set_import_error.set(None);
        }
        block_editor_plugin::PastedImage::Failed(error) => {
            state.set_import_error.set(Some(error));
        }
        block_editor_plugin::PastedImage::Empty => {
            request_paste();
        }
    }
}
