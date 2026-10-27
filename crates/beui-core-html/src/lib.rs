pub mod automate;
pub mod canvas;
pub mod css;
pub mod document;
pub mod drag_board;
pub mod dom;
pub mod drawing;
pub mod files;
pub mod focus;
pub mod fonts;
pub mod frame;
pub mod ime;
pub mod interactive;
pub mod keys;
pub mod measure;
pub mod node;
pub mod nodes;
pub mod overlay;
pub mod page;
pub mod portal;
pub mod rows;
pub mod run;
pub mod scroll;
pub mod tree;

pub use beui_html;
pub use node::{NodeId, NodeRef};

fn global_signal<T: 'static>(value: T) -> reactive::Signal<T> {
    let owned = reactive::OwnedSignal::new(value);
    let handle = owned.handle();
    std::mem::forget(owned);
    handle
}
