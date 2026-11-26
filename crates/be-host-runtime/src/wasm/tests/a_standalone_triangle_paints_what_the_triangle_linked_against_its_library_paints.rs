use super::*;

/// A module buck built (crates/be-host-runtime/BUCK names it).
fn module(variable: &str) -> Vec<u8> {
    let path = std::env::var(variable).unwrap_or_else(|_| {
        panic!(
            "{variable} names a module buck built, which crates/be-host-runtime/BUCK sets: \
             run this test with ./scripts/test, not the harness binary on its own"
        )
    });
    std::fs::read(&path).unwrap_or_else(|error| panic!("{path} could not be read: {error}"))
}

const TRIANGLE: &str = "5254d0a5-ae87-4afe-8458-50fde9cc0d2b";

#[test]
fn a_standalone_triangle_paints_what_the_triangle_linked_against_its_library_paints() {
    let editor = uuid::Uuid::parse_str(TRIANGLE).unwrap().into_bytes();
    let (linked, standalone, library) = (
        module("TRIANGLE_WASM"),
        module("TRIANGLE_STANDALONE_WASM"),
        module("BASE_LIBRARY_WASM"),
    );
    let (device, queue) = device();
    let host = Host::new(device.clone(), queue.clone()).unwrap();
    let linked = host
        .load_bytes_with_libraries(&linked, |_| Ok(library.clone()))
        .expect("the triangle linked against its library loads");
    let standalone = host
        .load_bytes(&standalone)
        .expect("the standalone triangle loads as it is");

    let linked_against = painted(&device, &queue, linked, editor);
    let alone = painted(&device, &queue, standalone, editor);

    let first = &linked_against[..4];
    assert!(
        linked_against.chunks(4).any(|pixel| pixel != first),
        "the triangle drew something on its background"
    );
    assert!(
        alone == linked_against,
        "the standalone triangle paints what the linked one does"
    );
}
