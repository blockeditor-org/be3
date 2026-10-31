use super::*;

const WIT: &str = "
package be3:test@0.1.0;

interface calls {
    ping: async func() -> u32;
    notify: func(count: u32);
}

world test {
    import calls;
}
";

#[test]
fn a_network_world_refuses_a_sync_func() {
    assert!(generated(WIT, false).is_ok());
    let error = generated(WIT, true).unwrap_err();
    assert!(
        error.contains("be3:test/calls@0.1.0#notify: a network world has no sync funcs"),
        "{error}"
    );
    let asynchronous = WIT.replace("notify: func", "notify: async func");
    let generated = generated(&asynchronous, true).unwrap();
    assert!(generated.contains("pub mod client"), "{generated}");
    assert!(!generated.contains("pub mod guest"), "{generated}");
}
