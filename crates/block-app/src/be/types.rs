use be_block::{
    CheckoutContent, DisplaySettingsContent, EditorViewContent, InputRemapperContent,
    InputSettingsContent, RepositoryContent, SettingsContent, SettingsProfileContent,
};
use block_guest_abi::{Flow, Guest, Waker};
use block_type_guest::{Module as TypeModule, Registration};

pub(super) use be_types_host::{Modules, Source, in_process};
pub(crate) use be_types_host::{is_known, kind_of};

pub(crate) struct HostTypes(TypeModule);

pub(crate) fn host_types() -> TypeModule {
    TypeModule::new(vec![
        Registration::of::<DisplaySettingsContent>(),
        Registration::of::<InputSettingsContent>(),
        Registration::of::<InputRemapperContent>(),
        Registration::of::<SettingsContent>().undo::<SettingsContent>(),
        Registration::of::<SettingsProfileContent>().undo::<SettingsProfileContent>(),
        Registration::of::<EditorViewContent>(),
        Registration::of::<RepositoryContent>(),
        Registration::of::<CheckoutContent>(),
    ])
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

pub(super) const BUILTIN: &str = "builtin-types";

pub(super) async fn sources() -> Vec<Source> {
    vec![builtin().await, in_process::<HostTypes>("host-types")]
}

#[cfg(not(test))]
async fn staged_tag() -> Result<block_type_api::SessionTag, String> {
    let name = format!("{BUILTIN}.sha256");
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
struct Crashing(builtin_types::BlockTypes);

#[cfg(test)]
impl Guest for Crashing {
    fn start(waker: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self(builtin_types::BlockTypes::start(waker, outbound)?))
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

#[cfg(test)]
async fn builtin() -> Source {
    in_process::<Crashing>(BUILTIN)
}

#[cfg(all(not(test), not(target_arch = "wasm32")))]
async fn builtin() -> Source {
    let entry = format!("{BUILTIN}.wasm");
    be_types_host::modules::wasm(BUILTIN, staged_tag().await, move |host| {
        host.load_from(&entry, crate::editors::plugin::discovery::read)
    })
}

#[cfg(all(not(test), target_arch = "wasm32"))]
async fn builtin() -> Source {
    be_types_host::modules::web(BUILTIN, format!("{BUILTIN}.wasm"), staged_tag().await)
}
