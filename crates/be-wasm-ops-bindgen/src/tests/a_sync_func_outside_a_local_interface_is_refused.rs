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
fn a_sync_func_outside_a_local_interface_is_refused() {
    for network in [false, true] {
        let error = generated(WIT, network).unwrap_err();
        assert!(
            error
                .contains("be3:test/calls@0.1.0#notify: a func is answered on the caller's thread"),
            "{error}"
        );
    }
    let asynchronous = WIT.replace("notify: func", "notify: async func");
    assert!(generated(&asynchronous, false).is_ok());
    let generated = generated(&asynchronous, true).unwrap();
    assert!(generated.contains("pub mod client"), "{generated}");
    assert!(!generated.contains("pub mod guest"), "{generated}");
}
