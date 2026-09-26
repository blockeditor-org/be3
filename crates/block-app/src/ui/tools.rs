use beui::reactive::{
    Func, Memo, clone, component, create_effect, create_memo, create_signal, untrack, view,
};
use beui::styled::DockArea;
use beui::unstyled::{DockState, TabId};
use beui::{NodeId, Rect, pos2, vec2};

use super::debug::{
    ClientPanel, DebugCommand, DebugWindow, PerformancePanel, PluginsPanel, VersionPanel,
};
use super::dialogs::{AboutPanel, InvitePanel};
use super::{AppViewStore, UiCommand, send};
use crate::surfaces::{HostSurface, SurfaceId};

const WORKSPACE: TabId = TabId::new(1);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Debug(DebugWindow),
    Invite,
    About,
}

impl Tool {
    const ALL: [Tool; 6] = [
        Tool::Debug(DebugWindow::Client),
        Tool::Debug(DebugWindow::Performance),
        Tool::Debug(DebugWindow::Plugins),
        Tool::Debug(DebugWindow::Version),
        Tool::Invite,
        Tool::About,
    ];

    fn tab(self) -> TabId {
        let index = Self::ALL
            .iter()
            .position(|tool| *tool == self)
            .unwrap_or_default();
        TabId::new(2 + index as u64)
    }

    fn of(tab: TabId) -> Option<Tool> {
        Self::ALL.into_iter().find(|tool| tool.tab() == tab)
    }

    fn title(self) -> &'static str {
        match self {
            Tool::Debug(DebugWindow::Client) => "Block Stack State",
            Tool::Debug(DebugWindow::Performance) => "Performance",
            Tool::Debug(DebugWindow::Plugins) => "Plugins",
            Tool::Debug(DebugWindow::Version) => "App Version",
            Tool::Invite => "Invite member",
            Tool::About => "About",
        }
    }

    fn window(self) -> Rect {
        let (position, size) = match self {
            Tool::Debug(DebugWindow::Client) => (pos2(60.0, 60.0), vec2(760.0, 600.0)),
            Tool::Debug(DebugWindow::Performance) => (pos2(120.0, 100.0), vec2(520.0, 420.0)),
            Tool::Debug(DebugWindow::Plugins) => (pos2(150.0, 90.0), vec2(560.0, 440.0)),
            Tool::Debug(DebugWindow::Version) => (pos2(180.0, 110.0), vec2(640.0, 480.0)),
            Tool::Invite => (pos2(120.0, 96.0), vec2(380.0, 360.0)),
            Tool::About => (pos2(160.0, 120.0), vec2(420.0, 230.0)),
        };
        Rect::from_min_size(position, size)
    }

    fn open(self, view: &AppViewStore) -> Memo<bool> {
        let view = view.clone();
        create_memo(move || match self {
            Tool::Debug(window) => {
                let debug = view.debug.get();
                match window {
                    DebugWindow::Client => debug.client.is_some(),
                    DebugWindow::Performance => debug.performance.is_some(),
                    DebugWindow::Plugins => debug.plugins.is_some(),
                    DebugWindow::Version => debug.version.is_some(),
                }
            }
            Tool::Invite => view.invite.get().is_some(),
            Tool::About => view.about.get(),
        })
    }

    fn close(self) {
        send(match self {
            Tool::Debug(window) => UiCommand::Debug(DebugCommand::Close(window)),
            Tool::Invite => UiCommand::CloseInvite,
            Tool::About => UiCommand::About(false),
        });
    }
}

#[component]
pub(super) fn WorkspaceDock(view: AppViewStore) -> NodeId {
    let (state, set_state) = create_signal(DockState::new([WORKSPACE]));
    for tool in Tool::ALL {
        let open = tool.open(&view);
        create_effect(clone!(set_state -> move || {
            let open = open.get();
            untrack(|| {
                set_state.update(|state| match (open, state.contains(tool.tab())) {
                    (true, false) => {
                        state.open_window(tool.window(), vec![tool.tab()]);
                    }
                    (false, true) => {
                        state.remove(tool.tab());
                    }
                    _ => {}
                });
            });
        }));
    }
    let status = view.status.clone();
    let title = Func::new(move |tab: TabId| match Tool::of(tab) {
        Some(tool) => tool.title().to_owned(),
        None => status.get().workspace,
    });
    view! {
        <DockArea
            state={state}
            title={title}
            closable={Func::new(|tab: TabId| tab != WORKSPACE)}
            on_change={move |next: DockState| set_state.set(next)}
            on_close={|tab: TabId| {
                if let Some(tool) = Tool::of(tab) {
                    tool.close();
                }
            }}
        >
            {move |tab: TabId| {
                let view = view.clone();
                let debug = view.debug.clone();
                match Tool::of(tab) {
                    None => view! {
                        <HostSurface id=SurfaceId::Main />
                    },
                    Some(Tool::Debug(DebugWindow::Client)) => {
                        let client = create_memo(move || debug.get().client);
                        view! {
                            <ClientPanel client />
                        }
                    }
                    Some(Tool::Debug(DebugWindow::Performance)) => {
                        let performance = create_memo(move || debug.get().performance);
                        view! {
                            <PerformancePanel performance />
                        }
                    }
                    Some(Tool::Debug(DebugWindow::Plugins)) => {
                        let plugins = create_memo(move || debug.get().plugins);
                        view! {
                            <PluginsPanel plugins />
                        }
                    }
                    Some(Tool::Debug(DebugWindow::Version)) => {
                        let version = create_memo(move || debug.get().version);
                        view! {
                            <VersionPanel version />
                        }
                    }
                    Some(Tool::Invite) => view! {
                        <InvitePanel view />
                    },
                    Some(Tool::About) => view! {
                        <AboutPanel />
                    },
                }
            }}
        </DockArea>
    }
}
