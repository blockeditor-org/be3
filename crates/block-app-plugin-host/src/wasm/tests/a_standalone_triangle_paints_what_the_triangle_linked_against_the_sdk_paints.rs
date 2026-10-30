use super::*;

const LINKED: &[u8] = include_bytes!(env!("TRIANGLE_WASM"));
const STANDALONE: &[u8] = include_bytes!(env!("TRIANGLE_STANDALONE_WASM"));
const SDK: &[u8] = include_bytes!(env!("PLUGIN_SDK_WASM"));
const TRIANGLE: &str = "5254d0a5-ae87-4afe-8458-50fde9cc0d2b";

#[test]
fn a_standalone_triangle_paints_what_the_triangle_linked_against_the_sdk_paints() {
    let editor = uuid::Uuid::parse_str(TRIANGLE).unwrap().into_bytes();
    let (device, queue) = device();
    let host = Host::new(device.clone(), queue.clone(), None)
        .unwrap()
        .with_direct(block_plugin_api::embed::IMPORTS);
    let linked = host
        .load_bytes_with_sdk(LINKED, |_| Ok(SDK.to_vec()))
        .expect("the triangle linked against the SDK loads");
    let standalone = host
        .load_bytes(STANDALONE)
        .expect("the standalone triangle loads as it is");

    let against_the_sdk = painted(&device, &queue, linked, editor);
    let alone = painted(&device, &queue, standalone, editor);

    let first = &against_the_sdk[..4];
    assert!(
        against_the_sdk.chunks(4).any(|pixel| pixel != first),
        "the triangle drew something on its background"
    );
    assert!(
        alone == against_the_sdk,
        "the standalone triangle paints what the linked one does"
    );
}
