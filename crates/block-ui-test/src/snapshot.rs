pub(crate) use paint_snapshot::Snapshot;

pub fn assert_snapshot(name: &str, snapshot: &Snapshot) {
    only_in_wasm();
    paint_snapshot::assert_accepted(
        name,
        snapshot,
        "./scripts/verify --plugin-tests, or ./scripts/buck test //crates/editors/<plugin>:test -- --env UPDATE_SNAPSHOTS=1",
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn only_in_wasm() {
    panic!(
        "this test compares a painting but was compiled natively. Paintings are only compared where tests run as wasm, an editor's or beui-demo's: put the test in one of those and run it with ./scripts/test NAME"
    );
}

#[cfg(target_arch = "wasm32")]
fn only_in_wasm() {}
