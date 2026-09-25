use std::rc::Rc;

use block_editor_beui::be_block::AudioContent;
use block_editor_beui::beui::icons::{ICON_AUDIO_FILE, ICON_PAUSE, ICON_PLAY_ARROW};
use block_editor_beui::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, NodeRef, Show, Spacer, clone, component, create_memo,
    create_signal, view,
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

#[component]
pub fn AudioView(editor: Editor) -> NodeId {
    let audio = editor.block_content::<AudioContent>();
    let source = audio.project(|audio| audio.header().source_name.clone());
    let (status, set_status) = create_signal(AudioStatus::default());
    let host = editor.host().clone();
    let waker = editor.host().waker();
    editor.each_frame(move || {
        let next = host.audio();
        if next.playing {
            waker.wake();
        }
        set_status.set(next);
    });

    let playing = create_memo(clone!(status -> move || status.with(|status| status.playing)));
    let glyph = create_memo(clone!(playing -> move || match playing.get() {
        true => ICON_PAUSE.to_owned(),
        false => ICON_PLAY_ARROW.to_owned(),
    }));
    let action = create_memo(clone!(playing -> move || match playing.get() {
        true => "Pause".to_owned(),
        false => "Play".to_owned(),
    }));
    let elapsed = create_memo(clone!(status -> move || {
        status.with(|status| {
            let duration = status
                .duration_micros
                .map_or_else(|| "--:--".to_owned(), format_micros);
            format!("{} / {duration}", format_micros(status.position_micros))
        })
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
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <IconButton
                                glyph={glyph}
                                label={action}
                                @test_id={"audio.play"}
                                on_click={toggle}
                            />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                        </List>
                        <Caption
                            content={elapsed}
                            align=TextAlign::Center
                            @test_id={"audio.position"}
                        />
                        <Show condition={failed}>
                            <Caption
                                content={reason}
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
    let polled = Rc::clone(&chooser);
    let host = editor.host().clone();
    let block = editor.block_id();
    editor.each_frame(move || {
        polled.poll(&host);
        if let Some(replacement) = polled.take() {
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
    let opened = editor.host().clone();
    let replace = move || chooser.open(&opened);
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
                <Caption content={reason} color={theme.danger.clone()} />
            </Show>
        </List>
    }
}
