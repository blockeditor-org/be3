use std::{sync::Arc, time::Duration};

use super::*;

#[test]
fn bytes_that_are_not_a_pure_guests_module_do_not_compile() {
    for (module, why) in [
        ("(module (memory 1 1 shared))", "a shared memory"),
        ("(module (memory i64 1))", "a 64-bit memory"),
    ] {
        let compiled = sandbox::compile(module.as_bytes().to_vec(), Arc::new(|| {}))
            .wait(Duration::from_secs(60))
            .expect("the compile finishes");
        let Err(error) = compiled else {
            panic!("{why} is not a pure guest's");
        };
        assert!(error.contains("pure guest"), "{why}: {error}");
    }
}
