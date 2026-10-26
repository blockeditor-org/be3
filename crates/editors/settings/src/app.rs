use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Editor, Loaded};

mod block;
mod devices;
mod organization;
mod setup;
mod ui;

use block::{BlockSettingsView, is_block_settings};
use block_editor_beui::BlockQuery;
use block_editor_beui::beui::ItemSize;
use block_editor_beui::beui::styled::{Frame, List, Space};
use ui::{SettingsProfileView, SettingsView};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 240.0;

pub struct SettingsApp;

#[component]
impl block_editor_beui::BeuiApp for SettingsApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let block = editor.block_id();
        let _listed = store(editor.blocks().watch(BlockQuery::Block(block)));
        let shown = create_memo(move || is_block_settings(editor, block));
        view! {
            <Root>
                <Loaded>
                    <List spacing=Space::None>
                        <Dynamic value={shown}>|shown: Option<Option<Uuid>>| {
                            match shown {
                                Some(Some(owner)) => view! {
                                    <BlockSettingsView
                                        @sizing=ItemSize::Percent(100.0)
                                        editor={editor}
                                        owner={owner}
                                    />
                                },
                                Some(None) => view! {
                                    <SettingsView
                                        @sizing=ItemSize::Percent(100.0)
                                        editor={editor}
                                    />
                                },
                                None => view! {
                                    <Frame @sizing=ItemSize::Percent(100.0) />
                                },
                            }
                        }</Dynamic>
                    </List>
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

pub struct SettingsProfileApp;

#[component]
impl block_editor_beui::BeuiApp for SettingsProfileApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <SettingsProfileView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
