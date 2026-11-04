use super::*;
use be_sdk_test::DocumentTest;
use beui::Vec2;
use beui::reactive::*;
use beui::styled::Root;
use beui_host::native::build;

use crate::ui::PerformancePanel;

#[test]
fn the_performance_panel_graphs_memory() {
    let _root = RootScope::new();
    let mut history = History {
        file: Some(PathBuf::from("/home/ada/.local/state/be-host/memory.tsv")),
        ..History::default()
    };
    let start = 1_700_000_000 / 60 * 60;
    for minute in 0..6 * 60u64 {
        // Asleep for half an hour, then a step up on waking.
        if (200..230).contains(&minute) {
            continue;
        }
        let woken = if minute >= 230 { 120 } else { 0 };
        let shared = 60 + (minute % 45);
        let wasm = 180 + minute / 6;
        let anonymous = 380 + minute / 2 + woken;
        history.record(Sample {
            at: start + minute * 60,
            usage: MemoryUsage {
                resident: (anonymous + shared + 90) << 20,
                anonymous: anonymous << 20,
                file: 90 << 20,
                shared: shared << 20,
                heap: (anonymous - wasm) << 20,
                heap_free: (40 + woken / 2) << 20,
                wasm: wasm << 20,
            },
            guests: vec![
                ("app be3:desktop".to_owned(), (wasm - 60) << 20),
                ("app be3:workspace".to_owned(), 60 << 20),
            ],
        });
    }
    let memory = history.view();
    let document = build(move || {
        let performance = create_memo(|| Some(Vec::new()));
        let memory = create_memo(move || memory.clone());
        view! {
            <Root>
                <PerformancePanel performance memory />
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(720.0, 560.0));
    test.frame(Vec::new());
    test.snapshot("the_performance_panel_graphs_memory");
}
