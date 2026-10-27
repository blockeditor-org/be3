use beui::native::WindowSize;
use beui::native::testing::{Showing, Stepped};
use beui::{Rect, native::Context, native::FreetypeFonts, pos2, vec2};

use super::*;
use crate::ui::{AppView, UnsentRow, UnsentView};
use beui::native::build;

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
            let menu = create_memo(|| {
                Some(AppMenuView {
                    opened: 1,
                    at: Some(Rect::from_min_size(pos2(16.0, 16.0), vec2(32.0, 32.0))),
                })
            });
            view! {
                <AppMenu status menu={menu} />
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
        let output = self.stepped.frame(Vec::new());
        let painted = beui::native::painting::painting(
            output.main(),
            SIZE,
            1.0,
            output.main().window.appearance.clear_color,
            beui::native::painting::Punches::Shown,
        )
        .unwrap();
        let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
        paint_snapshot::assert_accepted(name, &snapshot, "./scripts/verify");
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
