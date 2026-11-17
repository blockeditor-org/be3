use super::*;

use be_type_example_counter::CounterContent;

/// `be` reads its block types as be-host does, through be-host-stack's discovery, rather than
/// from anything compiled into it: here the counter's type module from the bundle BE_BUNDLE
/// names, as a `be` that was not deployed reads the one staged beside it. Its session tag is its
/// SHA-256, as be-host's is, and the module answers for its type.
#[tokio::test]
async fn be_reads_a_block_type_from_the_type_module_staged_beside_it() {
    be_lib_build_id::install("test");
    let sources: Vec<_> = be_host_stack::types::app_sources()
        .into_iter()
        .filter(|source| source.name == "be3.counter")
        .collect();
    assert_eq!(sources.len(), 1, "discovery found the counter's type module");
    let mut modules =
        be_block_type_host::Modules::new(sources, be_block_client::Clock::real());
    modules.load(0).await;

    let module = be_host_stack::discovery::open("counter_types.wasm")
        .and_then(|stored| stored.into_bytes())
        .expect("the counter's type module is staged");
    let staged: String = sha2::Digest::finalize(<sha2::Sha256 as sha2::Digest>::new_with_prefix(&module))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let tag: String = modules
        .tag(0)
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(tag, staged, "its session tag is its SHA-256");

    let blank = be_block_type_host::kind_of(CounterContent::CONTENT_TYPE)
        .expect("the counter's type is known")
        .blank;
    modules
        .describe(CounterContent::CONTENT_TYPE, &blank, Uuid::nil())
        .await
        .expect("the staged module describes a counter");
}
