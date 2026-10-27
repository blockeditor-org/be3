use beui::testing::Gui;
use std::time::Duration;

use beui::{Document, Key, Modifiers, Vec2};
use block_ui_test::DocumentTest;

use super::*;

const TIMEOUT: Duration = Duration::from_secs(15);
const WIDE: Vec2 = Vec2::new(1100.0, 800.0);

fn prompt(open: Signal<bool>, outcome: Signal<&'static str>) -> Document {
    build(move || {
        view! {
            <Root>
                <KeepChanges
                    open={open}
                    title="Keep these display settings?"
                    timeout=TIMEOUT
                    id={"keep-changes".to_owned()}
                    on_keep={move || {
                        outcome.set("kept");
                        open.set(false);
                    }}
                    on_revert={move || {
                        outcome.set("reverted");
                        open.set(false);
                    }}
                />
            </Root>
        }
    })
}

fn press(test: &mut DocumentTest, key: Key) {
    test.key(key, Modifiers::NONE);
    test.frame(Vec::new());
}

mod it_keeps_on_enter_and_reverts_on_escape;
mod it_reverts_once_its_countdown_runs_out;
