pub use paint_snapshot::Snapshot;

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

#[derive(Default)]
pub struct Recording {
    frames: Option<Snapshot>,
    out_of_bounds: Vec<String>,
}

pub trait Snapshots {
    fn capture(&mut self) -> Snapshot;

    fn out_of_bounds(&self) -> Vec<String>;

    fn recording(&mut self) -> &mut Recording;

    fn record(&mut self) {
        let frame = self.capture();
        let found = self.out_of_bounds();
        let recording = self.recording();
        recording.out_of_bounds.extend(found);
        match &mut recording.frames {
            Some(frames) => frames.append(frame),
            None => recording.frames = Some(frame),
        }
    }

    fn snapshot(&mut self, name: &str) {
        let painting = match self.recording().frames.take() {
            Some(frames) => frames,
            None => {
                let found = self.out_of_bounds();
                self.recording().out_of_bounds.extend(found);
                self.capture()
            }
        };
        assert_snapshot(name, &painting);
        let found = std::mem::take(&mut self.recording().out_of_bounds);
        crate::overflow::assert_fits(name, found);
    }
}
