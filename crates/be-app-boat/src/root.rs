use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_boat::BoatContent;
use uuid::Uuid;

mod list;
mod sign_in;
mod state;
mod ui;

pub(crate) use state::CACHE;
#[cfg(test)]
pub(crate) use state::Listed;

pub struct BoatRoot;

#[component]
impl be_sdk_beui::BeuiRoot for BoatRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded label="Loading boat.dev">
                    <ui::Boat editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&BoatContent::default()))
    }
}
