use beui_gallery_types::BeuiDemoContent;
use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Vec2;
use block_editor_beui::{Creation, EditorHost, GraphCommand, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::BeuiGalleryApp;

mod creating_a_beui_demo_makes_a_block_with_no_state;
mod the_demo_is_cleared_to_the_background_of_the_editors_theme;
mod the_editor_opens_every_page_of_the_catalog;
mod the_editor_shows_the_component_catalog;
