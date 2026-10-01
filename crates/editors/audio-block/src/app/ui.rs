use block_editor_beui::be_block::AudioContent;
use block_editor_beui::beui::icons::{ICON_AUDIO_FILE, ICON_PAUSE, ICON_PLAY_ARROW};
use std::time::Duration;

use block_editor_beui::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, NodeRef, Show, clone, component, create_effect,
    create_memo, create_signal, create_timer, now, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, IconButton, IconSized, use_theme,
};
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::{AudioStatus, Editor, FileChooser, Sidebar};

use super::{decode, filter, format_micros};

const PADDING: f32 = 12.0;
const SPACING: f32 = 8.0;
const ICON_SIZE: f32 = 48.0;
const SECOND_MICROS: u64 = 1_000_000;

fn played_to(status: &AudioStatus, since: Duration) -> u64 {
    let since = match status.playing {
        true => since.as_micros() as u64,
        false => 0,
    };
    let played = status.position_micros.saturating_add(since);
    status
        .duration_micros
        .map_or(played, |duration| played.min(duration))
}

#[component]
pub fn AudioView(editor: Editor) -> NodeId {
    let audio = editor.block_content::<AudioContent>();
    let source = audio.project(|audio| audio.header().source_name.clone());
    let status = editor.audio();

    let playing = create_memo(clone!(status -> move || status.with(|status| status.playing)));
    let glyph = create_memo(clone!(playing -> move || match playing.get() {
        true => ICON_PAUSE.to_owned(),
        false => ICON_PLAY_ARROW.to_owned(),
    }));
    let action = create_memo(clone!(playing -> move || match playing.get() {
        true => "Pause".to_owned(),
        false => "Play".to_owned(),
    }));
    let received = create_memo(clone!(status -> move || status.with(|_| now())));
    let (clock, set_clock) = create_signal(now());
    let position = create_memo(clone!(status received -> move || {
        status.with(|status| played_to(status, clock.get().saturating_duration_since(received.get())))
    }));
    let ticking = create_timer(clone!(status received -> move || {
        let at = now();
        set_clock.set(at);
        status.with_untracked(|status| {
            let played = played_to(status, at.saturating_duration_since(received.get_untracked()));
            let ended = status.duration_micros.is_some_and(|duration| played >= duration);
            (status.playing && !ended)
                .then(|| Duration::from_micros(SECOND_MICROS - played % SECOND_MICROS))
        })
    }));
    create_effect(clone!(status -> move || {
        if status.with(|status| status.playing) {
            ticking.restart(Duration::ZERO);
        } else {
            ticking.stop();
        }
    }));
    let elapsed = create_memo(clone!(status position -> move || {
        let duration = status.with(|status| status.duration_micros);
        let duration = duration.map_or_else(|| "--:--".to_owned(), format_micros);
        format!("{} / {duration}", format_micros(position.get()))
    }));
    let failed =
        create_memo(clone!(status -> move || status.with(|status| status.error.is_some())));
    let reason = create_memo(clone!(status -> move || {
        status.with(|status| status.error.clone().unwrap_or_default())
    }));

    let played = editor.host().clone();
    let block = editor.block_id();
    let toggle = move || played.play_audio(block);

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let panel = editor.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&content}
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <List spacing=SPACING>
                        <IconSized
                            glyph={ICON_AUDIO_FILE.to_owned()}
                            font_size=ICON_SIZE
                            color={theme.text.clone()}
                        />
                        <Body content={source} align=TextAlign::Center />
                        <IconButton
                            @sizing={ItemSize::Intrinsic.align(Align::Center)}
                            glyph={glyph}
                            label={action}
                            @test_id={"audio.play"}
                            on_click={toggle}
                        />
                        <Caption
                            content={elapsed}
                            align=TextAlign::Center
                            @test_id={"audio.position"}
                        />
                        <Show condition={failed}>
                            <Caption
                                content={reason.clone()}
                                align=TextAlign::Center
                                color={theme.danger.clone()}
                            />
                        </Show>
                    </List>
                </Frame>
                <Sidebar shown={chrome}>
                    <AudioPanel editor={panel} />
                </Sidebar>
            </List>
        </Frame>
    }
}

#[component]
fn AudioPanel(editor: Editor) -> NodeId {
    let replacing = editor.clone();
    let chooser = FileChooser::new(filter(), decode);
    let host = editor.host().clone();
    let block = editor.block_id();
    chooser.on_chosen(move |chooser| {
        if let Some(replacement) = chooser.take() {
            replacing.replace_content(block, &replacement);
            host.reset_audio(block);
        }
    });

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(clone!(busy read_only -> move || busy.get() || read_only.get()));
    let error = chooser.error();
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let replace = move || chooser.open();
    let theme = use_theme();
    view! {
        <List spacing=SPACING>
            <Heading content="Audio" />
            <Button
                label="Replace audio..."
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id={"audio.replace"}
                on_click={replace}
            />
            <Show condition={failed}>
                <Caption content={reason.clone()} color={theme.danger.clone()} />
            </Show>
        </List>
    }
}
