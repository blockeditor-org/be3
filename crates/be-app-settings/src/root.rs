use be_sdk_beui::beui::Vec2;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Editor, Loaded};

mod block;
mod devices;
mod organization;
mod setup;
mod ui;

use be_sdk_beui::BlockQuery;
use be_sdk_beui::beui::styled::{Frame, List, Space};
use block::{BlockSettingsView, is_block_settings};
use devices::DevicesPage;
use organization::{OfflinePage, OrganizationPage};
use ui::{SettingsProfileView, SettingsView};
use uuid::Uuid;

/// The `devices` editor in manifest.json: the user's `Settings` block, compared across devices.
pub(crate) const DEVICES_EDITOR: Uuid = Uuid::from_u128(0x9900d002_c861_49f7_a966_6243a3c4d1f3);
/// The `organization` editor in manifest.json: the organization's `Settings` block, as policies.
pub(crate) const ORGANIZATION_EDITOR: Uuid =
    Uuid::from_u128(0x1367cb00_ef42_407f_a3d3_67cbd1eaafae);

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 240.0;

pub struct SettingsRoot;

#[component]
impl be_sdk_beui::BeuiRoot for SettingsRoot {
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
                                    <ListItem grow=1.0>
                                        <BlockSettingsView editor={editor} owner={owner} />
                                    </ListItem>
                                },
                                Some(None) => view! {
                                    <ListItem grow=1.0>
                                        <SettingsView editor={editor} />
                                    </ListItem>
                                },
                                None => view! {
                                    <ListItem grow=1.0>
                                        <Frame />
                                    </ListItem>
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

pub struct SettingsProfileRoot;

#[component]
impl be_sdk_beui::BeuiRoot for SettingsProfileRoot {
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

pub struct DevicesRoot;

#[component]
impl be_sdk_beui::BeuiRoot for DevicesRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <DevicesPage editor={editor} />
                </Loaded>
            </Root>
        }
    }
}

pub struct OrganizationRoot;

#[component]
impl be_sdk_beui::BeuiRoot for OrganizationRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <OrganizationPage editor={editor} />
                </Loaded>
            </Root>
        }
    }
}

pub struct OfflineRoot;

#[component]
impl be_sdk_beui::BeuiRoot for OfflineRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <OfflinePage editor={editor} />
                </Loaded>
            </Root>
        }
    }
}
