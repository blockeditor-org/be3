use be_sdk_beui::{Drag, Pushed, SearchLocation, be_sdk_ui::BlockLabel};
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use beui_text_editor_core::{CursorLeftRightStop, EditorCommand};

use super::embeds::ResolvedEmbed;
use super::large_embed::{LargeEmbed, embed_is_live};
use super::state::{FocusedEmbed, State};
use beui::{Align, PickedFile};

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
        be_sdk_text::remote_carets(
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

    select_where_opened(state);
    caret_when_given_the_keyboard(state);
    let typing = state.typing;
    view! {
        <TextArea
            state={state.text}
            widgets={widgets}
            remote_cursors={remote_cursors}
            drop_caret={drop_caret}
            read_only={state.editor.read_only()}
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

/// An empty text given the keyboard (an editor that just made it handed it over, as a deck's
/// "Add notes" does) puts its caret in the text, so typing starts there.
fn caret_when_given_the_keyboard(state: State) {
    let keyboard = use_keyboard();
    let read_only = state.editor.read_only();
    let armed = StoredCell::new(false);
    let had = StoredCell::new(false);
    create_effect(move || {
        let given = keyboard.get();
        state.text.content().get();
        if !given {
            had.set(false);
            armed.set(false);
            return;
        }
        if !had.replace(true) {
            armed.set(true);
        }
        untrack(|| {
            if !armed.get() || !state.text.loaded() {
                return;
            }
            armed.set(false);
            let empty = state.text.bytes().is_empty();
            // Whatever else has the focus is what the window gave its first focusable (a window
            // that has just taken the keyboard does), which is not the person's choice; an
            // embed they pressed into is.
            if empty && !read_only.get() && state.focused_embed.get_untracked().is_none() {
                state.text.focus();
            }
        });
    });
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
    let image = be_type_image::ImageFile::from_file(file.name, file.data);
    let source_name = image.header().source_name.clone();
    let id = state.create_image_block(&image);
    state.insert_image_embed(id, &source_name);
    state.import_error.set(None);
}

/// Opened at a search hit's place (`editor-event.reveal`), the text selects the first place its
/// matched words occur and brings it into view, once the content has loaded and the position it
/// was left at is back, so the place it was opened at wins over it.
fn select_where_opened(state: State) {
    let revealed = state.editor.pushed(Pushed::Reveal);
    let wanted = create_signal(None::<SearchLocation>);
    create_effect(move || {
        revealed.get();
        if let Some(at) = untrack(|| state.editor.host().take_reveal()) {
            wanted.set(Some(at));
        }
    });
    let loaded = state.loaded;
    let restored = state.position_restored;
    create_effect(move || {
        let Some(at) = wanted.get() else {
            return;
        };
        if !loaded.get() || !restored.get() {
            return;
        }
        untrack(|| {
            wanted.set(None);
            // A text block is one document: a hit in any other is not this editor's to show.
            if !at.key.is_empty() {
                return;
            }
            if let Some(found) = find_words(&state.text.bytes(), &at.text) {
                state.text.select(found);
                state.text.reveal_cursor();
            }
        });
    });
}

/// Where `words` first occur in `text` as whole words, compared as the search index compares
/// them (`be_lib_search::fold`: Unicode case and compatibility folded), so a hit on "late" never
/// selects inside "chocolate".
pub(crate) fn find_words(text: &[u8], words: &str) -> Option<std::ops::Range<usize>> {
    let wanted: Vec<String> = be_lib_graphemes::words(words.as_bytes())
        .map(|range| be_lib_search::fold(&words[range]))
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let text = std::str::from_utf8(text).ok()?;
    let found: Vec<std::ops::Range<usize>> = be_lib_graphemes::words(text.as_bytes()).collect();
    found.windows(wanted.len()).find_map(|run| {
        run.iter()
            .zip(&wanted)
            .all(|(range, wanted)| be_lib_search::fold(&text[range.clone()]) == *wanted)
            .then(|| run[0].start..run[run.len() - 1].end)
    })
}
