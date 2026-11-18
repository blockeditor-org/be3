use std::cell::Cell;

use be_sdk_beui::worker::{App, Work};
use be_worker_map_tiles::Tiles;

use super::*;

thread_local! {
    static FAILED: Cell<bool> = const { Cell::new(false) };
}

/// The rasterizer, but failing the first job it is ever sent, which stops it.
struct FailsOnce(Tiles);

impl Work for FailsOnce {
    fn start(app: &mut App) -> Result<Self, String> {
        Tiles::start(app).map(Self)
    }

    fn receive(&mut self, message: Vec<u8>, app: &mut App) -> Result<(), String> {
        if !FAILED.replace(true) {
            return Err("the rasterizer could not draw this tile".to_owned());
        }
        self.0.receive(message, app)
    }
}

/// A job that stops the rasterizer fails only the tiles it was working on: the next tile starts
/// it anew and is drawn, as are all after it.
#[test]
fn a_tile_the_rasterizer_fails_on_fails_alone() {
    FAILED.set(false);
    let mut editor = editor();
    editor.serve_worker::<FailsOnce>(be_worker_map::MODULE);

    let mut served = Vec::new();
    loop {
        for (fetch, url) in requested_tiles(&mut editor) {
            serve(&mut editor, &fetch, ocean_tile());
            served.push(tile_test_id(&url));
        }
        let ids = served.clone();
        editor.settle_until("all but the failed tiles to be drawn", |editor| {
            editor.has_fetches()
                || ids.iter().filter(|id| tile_drawn(editor, id)).count() + 2 >= ids.len()
        });
        if !editor.has_fetches() {
            break;
        }
    }

    assert_eq!(served.len(), 16);
    let drawn = served.iter().filter(|id| tile_drawn(&editor, id)).count();
    assert_eq!(
        drawn, 14,
        "only the two tiles the rasterizer had when it stopped fail"
    );
    assert_eq!(
        editor.label("map.error"),
        "the rasterizer could not draw this tile",
        "the map says why"
    );
}
