use super::*;

const LINKED: &[u8] = include_bytes!(env!("TRIANGLE_WASM"));
const STANDALONE: &[u8] = include_bytes!(env!("TRIANGLE_STANDALONE_WASM"));
const LIBRARY: &[u8] = include_bytes!(env!("BASE_LIBRARY_WASM"));
const TRIANGLE: &str = "5254d0a5-ae87-4afe-8458-50fde9cc0d2b";

#[test]
fn a_standalone_triangle_paints_what_the_triangle_linked_against_its_library_paints() {
    let editor = uuid::Uuid::parse_str(TRIANGLE).unwrap().into_bytes();
    let (device, queue) = device();
    let host = Host::new(device.clone(), queue.clone(), None)
        .unwrap()
        .with_direct(block_plugin_api::embed::IMPORTS);
    let linked = host
        .load_bytes_with_libraries(LINKED, |_| Ok(LIBRARY.to_vec()))
        .expect("the triangle linked against its library loads");
    let standalone = host
        .load_bytes(STANDALONE)
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
