use be_sdk_beui::beui::styled::Root;

use be_sdk_beui::Loaded;
use be_sdk_beui::{Creation, Editor};
use beui::reactive::*;

use super::ui::LogicGridView;
use super::*;

pub struct LogicGridRoot;

#[component]
impl be_sdk_beui::BeuiRoot for LogicGridRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <LogicGridView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&LogicGridContent::default()))
    }

    fn exporters() -> Vec<be_sdk_beui::Exporter> {
        vec![dynamic_artifact::exporter().into()]
    }
}
