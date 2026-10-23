use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, icons::*};
use terminal_view::{Fit, TerminalView};

use super::offline::Offline;
use super::session::Session;

const BAR_PADDING: f32 = 8.0;

#[component]
pub(crate) fn TerminalBlock(editor: Editor) -> NodeId {
    let session = match Session::new(editor) {
        Ok(session) => session,
        Err(error) => {
            return view! {
                <Frame padding=24.0>
                    <Text wrap=true>{error}</Text>
                </Frame>
            };
        }
    };
    let online = create_memo(move || session.host.with(Option::is_some));
    let offline = create_memo(move || !online.get());
    view! {
        <List spacing=0.0>
            <Show condition={offline}>
                <Offline @sizing=ItemSize::Percent(100.0) editor={editor} />
            </Show>
            <Show condition={online}>
                <Running @sizing=ItemSize::Percent(100.0) session editor={editor} />
            </Show>
        </List>
    }
}

#[component]
fn Running(session: Session, editor: Editor) -> NodeId {
    let place = create_memo(move || {
        session.host.with(|host| match host {
            Some((_, host)) if host.user.is_empty() => format!("On {}", host.machine),
            Some((_, host)) => format!("{} on {}", host.user, host.machine),
            None => String::new(),
        })
    });
    let status = create_memo(move || {
        let read_only = editor.read_only().get();
        match (session.exited.get(), session.running.get(), read_only) {
            (Some(Some(code)), _, false) => format!("The shell exited with status {code}. Type to start a new one."),
            (Some(_), _, false) => "The shell exited. Type to start a new one.".to_owned(),
            (Some(_), _, true) => "The shell exited.".to_owned(),
            (None, None, _) => "Connecting…".to_owned(),
            (None, Some(_), true) => "View only".to_owned(),
            (None, Some(_), false) => String::new(),
        }
    });
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <List spacing=0.0>
            <Frame padding=BAR_PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Text style=TextStyle::CAPTION>
                        <Icon glyph={ICON_TERMINAL.to_owned()} />
                    </Text>
                    <Text style=TextStyle::CAPTION @test_id={"terminal.host"}>{place}</Text>
                    <Text style=TextStyle::CAPTION @test_id={"terminal.status"}>{status}</Text>
                </List>
            </Frame>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <TerminalView
                    screen={session.screen}
                    pastes=true
                    on_input={move |bytes: Vec<u8>| session.input(bytes)}
                    on_fit={move |fit: Fit| session.fit(fit)}
                />
            </Frame>
        </List>
    }
}
