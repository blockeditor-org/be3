extern crate beui_view as beui;

mod controls;
mod declare;
mod docking;
mod documents;
mod link;
pub mod styled;

pub use beui_dock_state::*;
pub use controls::{
    DockBarControl, DockBarDriver, DockMenus, DockTabControl, DockTabDriver, DockTabMenu, bar_menu,
    dock_menu, dock_menu_items, dock_shows_through, provide_dock_shows_through, use_dock_bar,
    use_dock_tab,
};
pub use declare::{
    DockBar, DockDialog, DockEntry, DockGroup, DockKey, DockNode, DockPane, DockSplit, DockTab,
    DockWindow,
};
pub use docking::{Docking, DockingLayout, DockingSnapshot, OwnedDockingLayout};
pub use documents::{
    DockDocument, DockDocumentBody, DockDocumentTabs, DockDocuments, DockOpened, DockSlot,
};
pub use link::{
    DockBarMove, DockBodies, DockDeclaration, DockDeclaredBar, DockDeclaredDialog,
    DockDeclaredGroup, DockDeclaredHost, DockDeclaredTab, DockDragStage, DockHostEvent,
    DockHostLink, DockHostSlot, DockHosted, DockJoin, DockKeeper, DockKept, DockRequest,
    use_dock_join,
};
