use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::beui::Vec2;
use be_sdk_beui::{Creation, EditorHost, GraphCommand, OwnedEditor};
use be_sdk_test::BeuiTest;
use be_type_example_beui_gallery::BeuiDemoContent;
use uuid::Uuid;

use crate::root::BeuiGalleryRoot;

mod creating_a_beui_demo_makes_a_block_with_no_state;
mod the_demo_is_cleared_to_the_background_of_the_editors_theme;
mod the_editor_opens_every_page_of_the_catalog;
mod the_editor_shows_the_component_catalog;
