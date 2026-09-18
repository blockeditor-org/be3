use std::cell::RefCell;
use std::rc::Rc;

use block_client::blocks::audio::{Audio, AudioOperation};
use block_editor_plugin::beui::icons::{ICON_AUDIO_FILE, ICON_PAUSE, ICON_PLAY_ARROW};
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, NodeRef, Show, Spacer, clone, component, create_memo,
    create_signal, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, IconButton, IconSized, use_theme,
};
use block_editor_plugin::beui::{NodeId, TextAlign};
use block_editor_plugin::{AudioStatus, Creation, Editor, FilePicker, Sidebar};

use super::{decode, filter, format_micros};

const PADDING: f32 = 12.0;
const SPACING: f32 = 8.0;
const ICON_SIZE: f32 = 48.0;

#[component]
pub fn AudioView(editor: Editor) -> NodeId {
    let audio = editor.block::<Audio>();
    let source = audio.project(|audio| audio.source_name().to_owned());
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
    let audio = editor.block::<Audio>();
    let picker = Rc::new(RefCell::new(FilePicker::default()));
    let (error, set_error) = create_signal(None::<String>);
    let (picking, set_picking) = create_signal(false);
    let host = editor.host().clone();
    let reset = editor.host().clone();
    let block = editor.block_id();
    let polled = Rc::clone(&picker);
    editor.each_frame(clone!(audio -> move || {
        let picked = polled.borrow_mut().poll(&host).map(|file| file.and_then(decode));
        set_picking.set(polled.borrow().is_open());
        match picked {
            Some(Ok(replacement)) => {
                audio.operate(AudioOperation::Replace { audio: replacement });
                reset.reset_audio(block);
                set_error.set(None);
            }
            Some(Err(reason)) => set_error.set(Some(reason)),
            None => {}
        }
    }));

    let read_only = editor.read_only();
    let blocked =
        create_memo(clone!(picking read_only -> move || picking.get() || read_only.get()));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let opened = editor.host().clone();
    let replace = move || picker.borrow_mut().open(&opened, filter());
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

#[component]
pub fn AudioCreation(creation: Creation) -> NodeId {
    let picker = Rc::new(RefCell::new(FilePicker::default()));
    let chosen = Rc::new(RefCell::new(None::<Audio>));
    let (name, set_name) = create_signal(None::<String>);
    let (error, set_error) = create_signal(None::<String>);
    let (picking, set_picking) = create_signal(false);
    creation.set_ready(false);

    let host = creation.host().clone();
    let polled = Rc::clone(&picker);
    let filled = Rc::clone(&chosen);
    let ready = creation.clone();
    creation.each_frame(move || {
        let picked = polled
            .borrow_mut()
            .poll(&host)
            .map(|file| file.and_then(decode));
        set_picking.set(polled.borrow().is_open());
        match picked {
            Some(Ok(audio)) => {
                set_name.set(Some(audio.source_name().to_owned()));
                *filled.borrow_mut() = Some(audio);
                set_error.set(None);
                ready.set_ready(true);
            }
            Some(Err(reason)) => {
                *filled.borrow_mut() = None;
                set_name.set(None);
                set_error.set(Some(reason));
                ready.set_ready(false);
            }
            None => {}
        }
    });

    let client = creation.client().clone();
    let made = Rc::clone(&chosen);
    creation.on_create(move || {
        let audio = made.borrow_mut().take().ok_or("no file was chosen")?;
        Ok(client.create_block(audio).id())
    });

    let opened = creation.host().clone();
    let choose = move || picker.borrow_mut().open(&opened, filter());
    let label = create_memo(clone!(name -> move || {
        name.get().unwrap_or_else(|| "No file chosen".to_owned())
    }));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        label="Choose file..."
                        variant=ButtonVariant::Secondary
                        disabled={picking}
                        @test_id={"audio.choose"}
                        on_click={choose}
                    />
                    <Caption @sizing=ItemSize::Percent(100.0) content={label} />
                </List>
                <Show condition={failed}>
                    <Caption
                        content={reason}
                        color={theme.danger.clone()}
                        @test_id={"audio.error"}
                    />
                </Show>
            </List>
        </Frame>
    }
}
