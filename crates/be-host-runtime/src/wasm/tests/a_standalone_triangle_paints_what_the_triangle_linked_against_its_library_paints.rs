use super::*;

/// A module buck compiled for wasmtime (crates/be-host-runtime/BUCK), which the host loads as it
/// is rather than compiling it: the base library alone is ten seconds of Cranelift.
fn compiled(variable: &str) -> Vec<u8> {
    let path = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} names a module"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{path} could not be read: {error}"))
}

const TRIANGLE: &str = "5254d0a5-ae87-4afe-8458-50fde9cc0d2b";

#[test]
fn a_standalone_triangle_paints_what_the_triangle_linked_against_its_library_paints() {
    let editor = uuid::Uuid::parse_str(TRIANGLE).unwrap().into_bytes();
    let (linked, standalone, library) = (
        compiled("TRIANGLE_CWASM"),
        compiled("TRIANGLE_STANDALONE_CWASM"),
        compiled("BASE_LIBRARY_CWASM"),
    );
    let (device, queue) = device();
    let host = Host::new(device.clone(), queue.clone(), None).unwrap();
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
