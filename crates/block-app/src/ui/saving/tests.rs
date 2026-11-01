use std::time::Duration;

use beui::native::WindowSize;
use beui::{native::Context, vec2};
use beui_host::native::FreetypeFonts;
use beui_host::native::testing::{Showing, Stepped};

use super::*;
use crate::ui::{AppView, Root as AppRoot, StatusView, SyncState};
use beui_host::native::build;
use block_app_desktop::desktop_host::{DesktopHost, Host};

fn saving(saved: bool, saves: u64) -> AppView {
    AppView {
        status: StatusView {
            sync: match saved {
                true => SyncState::Saved,
                false => SyncState::Syncing,
            },
            saves,
            ..Default::default()
        },
        ..Default::default()
    }
}

struct Saving {
    stepped: Stepped,
    view: AppViewStore,
}

impl Saving {
    fn start() -> Self {
        let mut view = None;
        let document = build(|| {
            crate::compositor::install();
            Host::create();
            crate::surfaces::create_handles();
            let store = AppViewStore::new(saving(false, 0));
            view = Some(store);
            view! {
                <beui::styled::Root>
                    <AppRoot view={store} />
                </beui::styled::Root>
            }
        });
        let window = WindowSize {
            size: vec2(1100.0, 720.0),
            scale: 1.0,
        };
        let mut stepped = Stepped::new(
            Context::new(FreetypeFonts::default()),
            Showing(document),
            window,
            None,
        );
        stepped.frame(Vec::new());
        Self {
            stepped,
            view: view.unwrap(),
        }
    }

    fn after(&mut self, wait: Duration, shown: AppView) -> bool {
        self.view.set(shown);
        self.stepped.frame(Vec::new());
        self.stepped.advance(wait);
        self.stepped.frame(Vec::new());
        self.stepped.frame(Vec::new());
        self.stepped
            .document()
            .find_test_id(&format!("toast.{TOAST_ID}"))
            .is_some()
    }
}

mod a_save_that_takes_a_while_shows_a_toast_until_it_finishes;
