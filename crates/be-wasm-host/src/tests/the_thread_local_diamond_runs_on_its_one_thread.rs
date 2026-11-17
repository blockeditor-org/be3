use super::*;

const BASE: &[u8] = include_bytes!(env!("TLS_DIAMOND_BASE_WASM"));
const LEFT: &[u8] = include_bytes!(env!("TLS_DIAMOND_LEFT_WASM"));
const RIGHT: &[u8] = include_bytes!(env!("TLS_DIAMOND_RIGHT_WASM"));
const APP: &[u8] = include_bytes!(env!("TLS_DIAMOND_APP_WASM"));

/// A single-threaded guest's thread-locals are plain statics
/// (decisions/guests_are_single_threaded.md), so the app reaches each library's through `GOT.mem`.
#[test]
fn the_thread_local_diamond_runs_on_its_one_thread() {
    let app = crate::engine::Shape::read(APP).unwrap();
    let reached: Vec<&str> = app
        .imports
        .iter()
        .filter(|import| import.module == "GOT.mem")
        .map(|import| import.name.as_str())
        .collect();
    for (local, library) in [("BASE", BASE), ("LEFT", LEFT), ("RIGHT", RIGHT)] {
        let library = crate::engine::Shape::read(library).unwrap();
        assert!(
            library
                .exports
                .iter()
                .any(|export| reached.contains(&export.name.as_str())),
            "the app reaches {local} without GOT.mem: {reached:?}"
        );
    }
    let mut app = host()
        .load_bytes_with_libraries(APP, |identity| {
            [BASE, LEFT, RIGHT]
                .into_iter()
                .find(|library| be_wasm_dylink::identity(library) == identity)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("no library is {identity}"))
        })
        .unwrap();
    let told = Told::listen(&app);
    app.start().unwrap();
    told.wait();
    app.step().unwrap();
    assert_eq!(app.take_outbound(), [[1]]);
}
