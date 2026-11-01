use beui::native::WindowSize;
use beui::{native::Context, vec2};
use beui_host::native::FreetypeFonts;
use beui_host::native::testing::{Showing, Stepped};

use super::*;
use crate::ui::{AppView, UnsentRow, UnsentView};
use beui_host::native::build;

const SIZE: beui::Vec2 = vec2(720.0, 640.0);

struct Shown {
    stepped: Stepped,
}

impl Shown {
    fn menu(sync: SyncState, unsent: usize) -> Self {
        let status = StatusView {
            sync,
            unsent,
            organization: "Acme".to_owned(),
            signed_in_as: "Signed in as Ada".to_owned(),
            ..Default::default()
        };
        Self::start(move || {
            let status = create_memo(move || status.clone());
            view! {
                <AppMenu status pick={Callback::new(send)} report={None} />
            }
        })
    }

    fn unsent(view: AppView) -> Self {
        Self::start(move || {
            let store = AppViewStore::new(view);
            view! {
                <Dialogs view={store} />
            }
        })
    }

    fn start(content: impl FnOnce() -> NodeId + 'static) -> Self {
        let document = build(move || {
            view! {
                <beui::styled::Root>{content()}</beui::styled::Root>
            }
        });
        let window = WindowSize {
            size: SIZE,
            scale: 1.0,
        };
        let mut stepped = Stepped::new(
            Context::new(FreetypeFonts::default()),
            Showing(document),
            window,
            None,
        );
        stepped.frame(Vec::new());
        stepped.frame(Vec::new());
        Self { stepped }
    }

    fn painted(&mut self, name: &str) {
        self.stepped.frame(Vec::new());
        self.stepped.snapshot(name);
    }
}

fn unsent_view() -> AppView {
    AppView {
        unsent: Some(UnsentView {
            rows: vec![
                UnsentRow {
                    block: uuid::Uuid::from_u128(1),
                    name: "Trip notes".to_owned(),
                    offerable: true,
                },
                UnsentRow {
                    block: uuid::Uuid::from_u128(2),
                    name: "Budget".to_owned(),
                    offerable: false,
                },
            ],
            discarding: None,
        }),
        ..Default::default()
    }
}

mod the_status_says_whether_changes_are_saved_syncing_offline_or_failed;
mod unsent_edits_can_be_restored_or_discarded;
