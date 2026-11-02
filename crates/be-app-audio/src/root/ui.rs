use be_sdk_beui::beui::icons::{ICON_FILE_HEADPHONE, ICON_PAUSE, ICON_PLAY};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use be_sdk_beui::beui::dock::styled::{
    DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign};
use be_sdk_beui::{AudioStatus, Editor, FileChooser, SIDE_PANEL_WIDTH};

use super::{decode, filter, format_micros};
use be_type_audio::AudioContent;
use beui::{Align, ItemSize};

const PADDING: Space = Space::Lg;
const SPACING: Space = Space::Md;
const SECOND_MICROS: u64 = 1_000_000;
const STAGE: &str = "stage";
const PANEL: &str = "panel";
const PANEL_WIDTH: f32 = SIDE_PANEL_WIDTH;

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
    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="audio" size={SplitSize::Second(PANEL_WIDTH)}>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="Track">
                        <Track @node_ref={&content} editor={editor} />
                    </DockTab>
                </DockPane>
                <Show condition={chrome}>
                    <DockPane id="panel" locked=true>
                        <DockTab id=PANEL title="Audio">
                            <AudioPanel editor={editor} />
                        </DockTab>
                    </DockPane>
                </Show>
            </DockSplit>
        </Docking>
    }
}

#[component]
fn Track(editor: Editor) -> NodeId {
    let audio = editor.block_content::<AudioContent>();
    let source = audio.project(|audio| audio.header().source_name.clone());
    let status = editor.audio();

    let playing = create_memo(move || status.with(|status| status.playing));
    let glyph = create_memo(move || match playing.get() {
        true => ICON_PAUSE,
        false => ICON_PLAY,
    });
    let action = create_memo(move || match playing.get() {
        true => "Pause".to_owned(),
        false => "Play".to_owned(),
    });
    let received = create_memo(move || status.with(|_| now()));
    let clock = create_signal(now());
    let position = create_memo(move || {
        status.with(|status| {
            played_to(
                status,
                clock.get().saturating_duration_since(received.get()),
            )
        })
    });
    let ticking = create_timer(move || {
        let at = now();
        clock.set(at);
        status.with_untracked(|status| {
            let played = played_to(
                status,
                at.saturating_duration_since(received.get_untracked()),
            );
            let ended = status
                .duration_micros
                .is_some_and(|duration| played >= duration);
            (status.playing && !ended)
                .then(|| Duration::from_micros(SECOND_MICROS - played % SECOND_MICROS))
        })
    });
    create_effect(move || {
        if status.with(|status| status.playing) {
            ticking.restart(Duration::ZERO);
        } else {
            ticking.stop();
        }
    });
    let elapsed = create_memo(move || {
        let duration = status.with(|status| status.duration_micros);
        let duration = duration.map_or_else(|| "--:--".to_owned(), format_micros);
        format!("{} / {duration}", format_micros(position.get()))
    });
    let failed = create_memo(move || status.with(|status| status.error.is_some()));
    let reason =
        create_memo(move || status.with(|status| status.error.clone().unwrap_or_default()));

    let host = editor.host();
    let loaded = Rc::new(Cell::new(None::<u64>));
    create_effect({
        let host = host.clone();
        let loaded = Rc::clone(&loaded);
        move || {
            let revision = audio.revision().ok();
            if loaded.get().is_some_and(|held| Some(held) != revision) {
                loaded.set(None);
                host.reset_audio();
            }
        }
    });
    let toggle = move || {
        let Ok(revision) = audio.revision() else {
            return;
        };
        if loaded.get() != Some(revision) {
            let Ok((media_type, bytes)) =
                audio.read(|audio| (audio.header().media_type.clone(), audio.data().to_vec()))
            else {
                return;
            };
            host.load_audio(media_type, bytes);
            loaded.set(Some(revision));
        }
        host.play_audio();
    };

    view! {
        <Frame padding=PADDING>
            <List spacing=SPACING>
                <Text size=TextSize::Xl4 align=TextAlign::Center>
                    <Icon glyph=ICON_FILE_HEADPHONE />
                </Text>
                <Text align=TextAlign::Center>{source}</Text>
                <Button
                    icon_only=true
                    @sizing={ItemSize::Intrinsic.align(Align::Center)}
                    glyph={glyph}
                    label={action}
                    @test_id={"audio.play"}
                    on_click={toggle}
                />
                <Text style=TextStyle::CAPTION align=TextAlign::Center @test_id={"audio.position"}>
                    {elapsed}
                </Text>
                <Show condition={failed}>
                    <Text style=TextStyle::CAPTION align=TextAlign::Center color=TextColor::Danger>
                        {reason}
                    </Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn AudioPanel(editor: Editor) -> NodeId {
    let chooser = FileChooser::new(filter(), decode);
    let block = editor.block_id();
    chooser.on_chosen(move |chooser| {
        if let Some(replacement) = chooser.take() {
            editor.replace_content(block, &replacement);
        }
    });

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(move || busy.get() || read_only.get());
    let error = chooser.error();
    let failed = create_memo(move || error.get().is_some());
    let reason = create_memo(move || error.get().unwrap_or_default());
    let replace = move || chooser.open();
    view! {
        <Frame fill=ColorRole::Surface>
            <Scroll @test_id={"audio.panel"}>
                <Frame padding=PADDING>
                    <List spacing=SPACING>
                        <Button
                            label="Replace audio..."
                            variant=ButtonVariant::Secondary
                            disabled={blocked}
                            @test_id={"audio.replace"}
                            on_click={replace}
                        />
                        <Show condition={failed}>
                            <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
                        </Show>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}
