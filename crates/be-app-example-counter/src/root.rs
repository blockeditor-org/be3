use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_example_counter::CounterContent;
use uuid::Uuid;

mod ui;

use ui::Counter;

pub struct CounterRoot;

#[component]
impl be_sdk_beui::BeuiRoot for CounterRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <Counter editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CounterContent::default()))
    }
}

/// The counter's host component (guides/apps/host_services.md, "Host components"): once its
/// block's content arrives it adds one, which is all a headless host of it does
/// (plans/headless_apps.md).
pub struct CounterHost;

#[component]
impl be_sdk_beui::BeuiRoot for CounterHost {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        view! {
            <Root>
                <be_sdk_beui::beui::styled::Frame />
            </Root>
        }
    }

    fn host(editor: Editor) {
        let counter = editor.block_content::<CounterContent>();
        let added = std::cell::Cell::new(false);
        create_effect(move || {
            if counter.ready().is_ok() && !added.replace(true) {
                counter.operate(be_type_example_counter::Counter::add(1));
            }
        });
    }
}
