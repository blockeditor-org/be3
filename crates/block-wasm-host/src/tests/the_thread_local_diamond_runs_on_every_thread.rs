use super::*;

use std::sync::mpsc;
use std::time::Duration;

const BASE: &[u8] = include_bytes!(env!("TLS_DIAMOND_BASE_WASM"));
const LEFT: &[u8] = include_bytes!(env!("TLS_DIAMOND_LEFT_WASM"));
const RIGHT: &[u8] = include_bytes!(env!("TLS_DIAMOND_RIGHT_WASM"));
const PLUGIN: &[u8] = include_bytes!(env!("TLS_DIAMOND_PLUGIN_WASM"));

#[test]
fn the_thread_local_diamond_runs_on_every_thread() {
    let engine = wasmtime::Engine::default();
    let plugin = wasmtime::Module::new(&engine, PLUGIN).unwrap();
    let reached: Vec<&str> = plugin
        .imports()
        .filter(|import| import.module() == "GOT.tls")
        .map(|import| import.name())
        .collect();
    for (local, library) in [("BASE", BASE), ("LEFT", LEFT), ("RIGHT", RIGHT)] {
        let library = wasmtime::Module::new(&engine, library).unwrap();
        assert!(
            library
                .exports()
                .any(|export| reached.contains(&export.name())),
            "the plugin reaches {local} without GOT.tls: {reached:?}"
        );
    }
    let mut plugin = host()
        .load_bytes_with_libraries(PLUGIN, |identity| {
            [BASE, LEFT, RIGHT]
                .into_iter()
                .find(|library| wasm_dylink::identity(library) == identity)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("no library is {identity}"))
        })
        .unwrap();
    let (woken, wake) = mpsc::channel();
    plugin.on_wake(move || {
        let _ = woken.send(());
    });
    plugin.start().unwrap();
    wake.recv_timeout(Duration::from_secs(60))
        .expect("the spawned thread never woke the plugin");
    plugin.step().unwrap();
    assert_eq!(plugin.take_outbound(), [[1]]);
}
