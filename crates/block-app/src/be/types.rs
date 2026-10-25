use be_block::{
    BlockContent, CheckoutContent, DisplaySettingsContent, HomeContent, InputRemapperContent,
    InputSettingsContent, PrefixRegistryContent, RepositoryContent, SettingsContent,
    SettingsProfileContent, TagContent, TagsContent,
};
use block_guest_abi::{Flow, Guest, Waker};
use block_plugin_api::{BlockTypeDescriptor, ChildOperations, Press};
use block_type_guest::Module as TypeModule;
use uuid::Uuid;

pub(super) use be_types_host::{Modules, Source, in_process};
pub(crate) use be_types_host::{is_known, kind_of};

pub(crate) struct HostTypes(TypeModule);

fn host_types() -> TypeModule {
    TypeModule::new(host_types::registrations())
}

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

pub(crate) fn host_block_types() -> Vec<BlockTypeDescriptor> {
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
pub(crate) fn provided(content_type: Uuid) -> bool {
    crate::editors::plugin::discovery::block_type(content_type).is_some()
}

#[cfg(test)]
pub(crate) fn provided(content_type: Uuid) -> bool {
    host_types().knows(content_type) || first_party_types::knows(content_type)
}

impl Guest for HostTypes {
    fn start(_: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        let module = host_types();
        outbound.push(module.hello_frame());
        Ok(Self(module))
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.0.step(inbound, outbound);
        Ok(Flow::Running)
    }
}

#[cfg(not(test))]
pub(super) async fn sources() -> Vec<Source> {
    let mut sources = vec![in_process::<HostTypes>("host-types")];
    for manifest in crate::editors::plugin::discovery::manifests() {
        if let Some(module) = manifest.type_module() {
            let declared = manifest
                .block_types
                .iter()
                .map(|block_type| Uuid::from_bytes(block_type.content_type()))
                .collect();
            sources.push(
                staged(&manifest.identity.id, module)
                    .await
                    .declaring(declared),
            );
        }
    }
    sources
}

#[cfg(test)]
pub(super) async fn sources() -> Vec<Source> {
    let mut sources = vec![in_process::<HostTypes>("host-types")];
    sources.extend(first_party_types::sources().into_iter().map(
        |source| match source.name.as_str() {
            "be3.counter" => first_party_types::in_process::<Crashing>(
                "be3.counter",
                counter_types::registrations(),
            ),
            _ => source,
        },
    ));
    sources
}

#[cfg(not(test))]
async fn staged_tag(module: &str) -> Result<block_type_api::SessionTag, String> {
    let name = format!("{}.sha256", module.strip_suffix(".wasm").unwrap_or(module));
    #[cfg(not(target_arch = "wasm32"))]
    let read = crate::editors::plugin::discovery::read(&name);
    #[cfg(target_arch = "wasm32")]
    let read = crate::platform::http::get(name.clone(), Vec::new()).await;
    let hex = String::from_utf8(read?).map_err(|error| error.to_string())?;
    let hash = be_store::Hash::from_hex(hex.trim()).ok_or(format!("{name} holds no hash"))?;
    Ok(block_type_api::SessionTag(*hash.as_bytes()))
}

#[cfg(test)]
pub(crate) static CRASH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
struct Crashing(counter_types::BlockTypes);

#[cfg(test)]
impl Guest for Crashing {
    fn start(waker: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self(counter_types::BlockTypes::start(waker, outbound)?))
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
async fn staged(name: &str, module: &str) -> Source {
    let entry = module.to_owned();
    be_types_host::modules::wasm(name, staged_tag(module).await, None, move |host| {
        host.load_from(&entry, crate::editors::plugin::discovery::read)
    })
}

#[cfg(all(not(test), target_arch = "wasm32"))]
async fn staged(name: &str, module: &str) -> Source {
    be_types_host::modules::web(name, module.to_owned(), staged_tag(module).await)
}
