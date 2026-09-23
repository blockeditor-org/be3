use beui::reactive::{
    ClickCatcher, Focusable, ForEach, Frame, Func, List, Viewport, clone, component,
    component_rect, create_effect, create_memo, create_signal, on_cleanup, view,
};
use beui::styled::theme::use_theme;
use beui::styled::{
    Body, Button, ButtonVariant, Caption, DockArea, Heading, ListRow, Scroll, TextInput,
};
use beui::unstyled::{DockState, TabId};
use beui::{Align, Direction, ItemSize, NodeId};

use crate::clients::{ClientSignals, Clients, Command, LAUNCHER, tab_of, window_of};
use crate::state::WindowId;

const SHELL_PADDING: f32 = 8.0;
const PANEL_PADDING: f32 = 14.0;
const SPACING: f32 = 8.0;

#[component]
pub fn Workspace(clients: Clients) -> NodeId {
    let theme = use_theme();
    let state = clients.dock();
    let title = Func::new(clone!(clients -> move |tab: TabId| clients.title(tab)));
    let changed = clients.clone();
    let closed = clients.clone();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=SHELL_PADDING
            padding_vertical=SHELL_PADDING
        >
            <DockArea
                state
                title
                closable={Func::new(|tab: TabId| tab != LAUNCHER)}
                on_change={move |next: DockState| changed.set_dock(next)}
                on_close={move |tab: TabId| {
                    if let Some(id) = window_of(tab) {
                        closed.push(Command::Close(id));
                    }
                }}
            >
                {move |tab: TabId| {
                    let clients = clients.clone();
                    let window = window_of(tab).and_then(|id| Some((id, clients.signals(id)?)));
                    match window {
                        Some((id, signals)) => view! {
                            <ClientView clients id signals />
                        },
                        None => view! {
                            <Launcher clients />
                        },
                    }
                }}
            </DockArea>
        </Frame>
    }
}

#[component]
fn Launcher(clients: Clients) -> NodeId {
    let (command, set_command) = create_signal(String::new());
    let socket = format!("WAYLAND_DISPLAY={}", clients.socket());
    let order = clients.order();
    let launch = clone!(clients command set_command -> move || {
        let line = command.get_untracked();
        if !line.trim().is_empty() {
            clients.push(Command::Launch(line));
            set_command.set(String::new());
        }
    });
    let submit = launch.clone();
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=SPACING>
                <Heading content="Run a Wayland app" />
                <Caption content={socket} />
                <TextInput
                    @test_id={"compositor.command"}
                    value={command}
                    placeholder="Command, like foot or gtk4-demo"
                    label="Command"
                    on_change={move |line: String| set_command.set(line)}
                    on_submit={move |_: String| submit()}
                />
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        @test_id={"compositor.run"}
                        label="Run"
                        variant=ButtonVariant::Primary
                        on_click={launch}
                    />
                </List>
                <Heading content="Windows" />
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <ForEach keys={order}>
                        {move |id: WindowId| {
                            let clients = clients.clone();
                            view! {
                                <WindowRow clients id />
                            }
                        }}
                    </ForEach>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn WindowRow(clients: Clients, id: WindowId) -> NodeId {
    let title = create_memo(clone!(clients -> move || clients.title(tab_of(id))));
    view! {
        <ListRow
            @test_id={format!("compositor.window.{}", id.0)}
            on_click={move || clients.show(id)}
        >
            <Body content={title} />
        </ListRow>
    }
}

#[component]
fn ClientView(clients: Clients, id: WindowId, signals: ClientSignals) -> NodeId {
    let rect = component_rect();
    create_effect(clone!(clients -> move || clients.placed(id, rect.get())));
    let cursor = clients.cursor();
    let focus = clients.clone();
    let hover = clients.clone();
    on_cleanup(move || clients.unplaced(id));
    view! {
        <Focusable
            @test_id={format!("compositor.client.{}", id.0)}
            focused={signals.focused}
            on_focus_change={move |focused: bool| focus.focus(id, focused)}
            on_key={|_| true}
        >
            <ClickCatcher cursor on_hover_change={move |hovered: bool| hover.hover(id, hovered)}>
                <Viewport drawing={signals.drawing} />
            </ClickCatcher>
        </Focusable>
    }
}
