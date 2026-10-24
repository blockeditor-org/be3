use block_editor_beui::Editor;
use block_editor_beui::be_block::terminal::TerminalWheel;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, icons::*};
use terminal_view::{Fit, Lines, TerminalInput, TerminalView};

use super::offline::{Offline, SetupSteps};
use super::session::Session;

const BAR_PADDING: f32 = 8.0;
const BAR_HEIGHT: f32 = 48.0;
const SETUP_WIDTH: f32 = 440.0;

#[component]
pub(crate) fn TerminalBlock(editor: Editor) -> NodeId {
    let session = Session::new(editor);
    let setup = create_memo(move || session.host.with(Option::is_none) && session.empty());
    let running = create_memo(move || !setup.get());
    view! {
        <List spacing=0.0>
            <Show condition={setup}>
                <Offline @sizing=ItemSize::Percent(100.0) editor={editor} />
            </Show>
            <Show condition={running}>
                <Running @sizing=ItemSize::Percent(100.0) session editor={editor} />
            </Show>
        </List>
    }
}

#[component]
fn Running(session: Session, editor: Editor) -> NodeId {
    let connected = create_memo(move || session.host.with(Option::is_some));
    let disconnected = create_memo(move || !connected.get());
    let place = create_memo(move || {
        session.host.with(|host| match host {
            Some((_, host)) if host.user.is_empty() => format!("On {}", host.machine),
            Some((_, host)) => format!("{} on {}", host.user, host.machine),
            None => String::new(),
        })
    });
    let status = create_memo(move || {
        let read_only = editor.read_only().get();
        let typing = connected.get() && !read_only;
        match session.shell.get() {
            Some(shell) if !shell.running => match (shell.status, typing) {
                (Some(code), true) => {
                    format!("The shell exited with status {code}. Type to start a new one.")
                }
                (None, true) => "The shell exited. Type to start a new one.".to_owned(),
                (Some(code), false) => format!("The shell exited with status {code}."),
                (None, false) => "The shell exited.".to_owned(),
            },
            None if connected.get() => "Connecting…".to_owned(),
            _ if read_only => "View only".to_owned(),
            _ => String::new(),
        }
    });
    let peek = create_signal(false);
    let alternate = session.alternate_active;
    create_effect(move || {
        if !alternate.get() {
            peek.set(false);
        }
    });
    let program = create_memo(move || alternate.get() && !peek.get());
    let history = create_memo(move || match program.get() {
        true => Lines::default(),
        false => session.history.get(),
    });
    let rows = create_memo(move || match program.get() {
        true => session.alternate.get(),
        false => session.screen.get(),
    });
    let cursor = create_memo(move || {
        let cursor = session.cursor.get();
        (cursor.visible && !peek.get()).then_some(cursor)
    });
    let input = create_memo(move || editor.editable().get() && connected.get() && !peek.get());
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <List spacing=0.0>
            <Frame padding=BAR_PADDING height={Extent::at_least(BAR_HEIGHT)} align={Align2::new(Align::Stretch, Align::Center)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Text style=TextStyle::CAPTION>
                        <Icon glyph={ICON_TERMINAL.to_owned()} />
                    </Text>
                    <Show condition={connected}>
                        <Text style=TextStyle::CAPTION @test_id={"terminal.host"}>{place}</Text>
                    </Show>
                    <Show condition={disconnected}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Text style=TextStyle::CAPTION color=TextColor::Warning>
                                <Icon glyph={ICON_UNPLUG.to_owned()} />
                            </Text>
                            <Text style=TextStyle::CAPTION @test_id={"terminal.offline"}>
                                "No machine is running this terminal"
                            </Text>
                            <Popover
                                label="How to start one"
                                glyph={ICON_INFO.to_owned()}
                                variant=ButtonVariant::Ghost
                                max_width=SETUP_WIDTH
                                @test_id={"terminal.setup"}
                            >|_|
                                <SetupSteps editor={editor} />
                            </Popover>
                        </List>
                    </Show>
                    <Text style=TextStyle::CAPTION @test_id={"terminal.status"}>{status}</Text>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Show condition={alternate}>
                        <ToggleButton
                            label="Main screen"
                            glyph={ICON_SCROLL_TEXT.to_owned()}
                            pressed={peek}
                            @test_id={"terminal.peek"}
                        />
                    </Show>
                </List>
            </Frame>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <TerminalView
                    history
                    rows
                    cursor
                    colors={session.colors}
                    input
                    wheel={program}
                    on_input={move |input: TerminalInput| session.input(input)}
                    on_wheel={move |wheel: TerminalWheel| session.wheel(wheel)}
                    on_fit={move |fit: Fit| session.fit(fit)}
                />
            </Frame>
        </List>
    }
}
