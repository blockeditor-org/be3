use std::{sync::Arc, time::Duration};

use super::*;

#[test]
fn bytes_that_are_not_a_pure_guests_module_do_not_compile() {
    let compiled = sandbox::compile(
        br#"(module (memory 1 1 shared))"#.to_vec(),
        Arc::new(|| {}),
    )
    .wait(Duration::from_secs(60))
    .expect("the compile finishes");
    let Err(error) = compiled else {
        panic!("a shared memory is not a pure guest's");
    };
    assert!(error.contains("pure guest"), "{error}");
}
