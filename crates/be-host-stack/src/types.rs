use be_block::{
    DisplaySettingsContent, HomeContent, OfflineSettingsContent, OrganizationPoliciesContent,
    PrefixRegistryContent, SettingsContent, SettingsProfileContent, TagContent, TagsContent,
};
use be_block_content::BlockContent;
use be_block_input::{InputRemapperContent, InputSettingsContent};
use be_block_repository::{CheckoutContent, RepositoryContent};
use be_sdk_api::{BlockTypeDescriptor, ChildOperations, Press};
#[cfg(test)]
use be_wasm_guest_abi::{Flow, Guest, Waker};
use uuid::Uuid;

pub use be_block_type_host::Source;
pub(super) use be_block_type_host::{Modules, in_process};

fn described(content_type: Uuid, name: &str, icon: &str, color: [u8; 3]) -> BlockTypeDescriptor {
    BlockTypeDescriptor {
        block_type: content_type.into_bytes(),
        display_name: name.to_owned(),
        color: Some(color),
        icon_codepoint: icon.to_owned(),
        children: ChildOperations::default(),
        press: Press::Open,
        plain_file: false,
    }
}

pub fn host_block_types() -> Vec<BlockTypeDescriptor> {
    const BLUE: [u8; 3] = [65, 145, 210];
    const GREEN: [u8; 3] = [44, 158, 128];
    const PURPLE: [u8; 3] = [155, 112, 211];
    const TEAL: [u8; 3] = [64, 155, 173];
    const GOLD: [u8; 3] = [172, 145, 47];
    vec![
        described(CheckoutContent::CONTENT_TYPE, "Checkout", "\u{e125}", BLUE),
        described(
            DisplaySettingsContent::CONTENT_TYPE,
            "Display",
            "\u{e603}",
            BLUE,
        ),
        described(
            InputRemapperContent::CONTENT_TYPE,
            "Input Remaps",
            "\u{e284}",
            PURPLE,
        ),
        described(
            InputSettingsContent::CONTENT_TYPE,
            "Input",
            "\u{e284}",
            GREEN,
        ),
        described(
            OfflineSettingsContent::CONTENT_TYPE,
            "Offline",
            "\u{e154}",
            TEAL,
        ),
        described(
            OrganizationPoliciesContent::CONTENT_TYPE,
            "Organization policies",
            "\u{e154}",
            TEAL,
        ),
        described(
            RepositoryContent::CONTENT_TYPE,
            "Repository",
            "\u{e40a}",
            GOLD,
        ),
        described(SettingsContent::CONTENT_TYPE, "Settings", "\u{e154}", TEAL),
        described(
            PrefixRegistryContent::CONTENT_TYPE,
            "Database prefixes",
            "\u{e154}",
            TEAL,
        ),
        described(TagsContent::CONTENT_TYPE, "Tags", "\u{e154}", TEAL),
        described(TagContent::CONTENT_TYPE, "Tag", "\u{e154}", TEAL),
        described(HomeContent::CONTENT_TYPE, "Home", "\u{e154}", TEAL),
        described(
            SettingsProfileContent::CONTENT_TYPE,
            "Settings Profile",
            "\u{e154}",
            GOLD,
        ),
    ]
}

#[cfg(not(test))]
pub fn provided(content_type: Uuid) -> bool {
    #[cfg(feature = "testing")]
    if let Some(types) = crate::testing::in_process() {
        return be_sdk_type_host::block_types().knows(content_type) || (types.knows)(content_type);
    }
    crate::discovery::block_type(content_type).is_some()
}

#[cfg(test)]
pub fn provided(content_type: Uuid) -> bool {
    be_sdk_type_host::block_types().knows(content_type) || be_type_bundle::knows(content_type)
}

#[cfg(not(test))]
pub(super) async fn sources() -> Vec<Source> {
    let mut sources = vec![in_process::<be_sdk_type_host::BlockTypes>("host-types")];
    #[cfg(feature = "testing")]
    if let Some(types) = crate::testing::in_process() {
        sources.extend((types.sources)());
        return sources;
    }
    #[cfg(not(target_arch = "wasm32"))]
    sources.extend(app_sources());
    #[cfg(target_arch = "wasm32")]
    for (app, module, declared) in type_modules() {
        sources.push(staged(&app, &module).await.declaring(declared));
    }
    sources
}

/// The type module of each app discovery found (`crate::discovery::manifests`), each read
/// through `crate::discovery::open` when it first starts: from the bundle beside the
/// executable, or from what `crate::discovery::use_assets` serves. be-host's stack runs these
/// beside its own types; `be` runs only these (guides/type_modules.md, "Hosts").
#[cfg(all(not(test), not(target_arch = "wasm32")))]
pub fn app_sources() -> Vec<Source> {
    type_modules()
        .map(|(app, module, declared)| staged(&app, &module).declaring(declared))
        .collect()
}

/// Each found app with a type module: its id, the module's file and the types it declares.
#[cfg(not(test))]
fn type_modules() -> impl Iterator<Item = (String, String, Vec<Uuid>)> {
    crate::discovery::manifests()
        .into_iter()
        .filter_map(|manifest| {
            let module = manifest.type_module()?.to_owned();
            let declared = manifest
                .block_types
                .iter()
                .map(|block_type| Uuid::from_bytes(block_type.content_type()))
                .collect();
            Some((manifest.identity.id.clone(), module, declared))
        })
}

#[cfg(test)]
pub(super) async fn sources() -> Vec<Source> {
    let mut sources = vec![in_process::<be_sdk_type_host::BlockTypes>("host-types")];
    sources.extend(be_type_bundle::sources().into_iter().map(
        |source| match source.name.as_str() {
            "be3.counter" => be_type_bundle::in_process::<Crashing>(
                "be3.counter",
                be_type_example_counter::registrations(),
            ),
            _ => source,
        },
    ));
    sources
}

/// The session tag staged beside `module` (be-tool-buck's stage writes NAME.sha256).
#[cfg(not(test))]
fn tag_name(module: &str) -> String {
    format!("{}.sha256", module.strip_suffix(".wasm").unwrap_or(module))
}

#[cfg(not(test))]
fn parse_tag(name: &str, read: Result<Vec<u8>, String>) -> Result<be_sdk_type_api::SessionTag, String> {
    let hex = String::from_utf8(read?).map_err(|error| error.to_string())?;
    let hash = be_block_store::Hash::from_hex(hex.trim()).ok_or(format!("{name} holds no hash"))?;
    Ok(be_sdk_type_api::SessionTag(*hash.as_bytes()))
}

#[cfg(all(not(test), not(target_arch = "wasm32")))]
fn staged_tag(module: &str) -> Result<be_sdk_type_api::SessionTag, String> {
    let name = tag_name(module);
    let read = crate::discovery::open(&name).and_then(be_wasm_host::Stored::into_bytes);
    parse_tag(&name, read)
}

#[cfg(all(not(test), target_arch = "wasm32"))]
async fn staged_tag(module: &str) -> Result<be_sdk_type_api::SessionTag, String> {
    let name = tag_name(module);
    let read = be_host_base::platform::http::get(name.clone(), Vec::new()).await;
    parse_tag(&name, read)
}

#[cfg(test)]
pub static CRASH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
struct Crashing(be_type_example_counter::BlockTypes);

#[cfg(test)]
impl Guest for Crashing {
    fn start(waker: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self(be_type_example_counter::BlockTypes::start(
            waker, outbound,
        )?))
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        if CRASH.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return Err("the test stopped the type module".to_owned());
        }
        self.0.step(inbound, woken, outbound)
    }
}

#[cfg(all(not(test), not(target_arch = "wasm32")))]
fn staged(name: &str, module: &str) -> Source {
    let entry = module.to_owned();
    be_block_type_host::modules::wasm(name, staged_tag(module), move |host| {
        host.load_from(&entry, crate::discovery::open)
    })
}

#[cfg(all(not(test), target_arch = "wasm32"))]
async fn staged(name: &str, module: &str) -> Source {
    be_block_type_host::modules::web(name, module.to_owned(), staged_tag(module).await)
}
