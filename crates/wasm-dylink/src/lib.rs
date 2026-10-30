#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SDK_EXPORT_PREFIX: &str = "__be3_sdk_";
pub const LAYOUT_EXPORT_PREFIX: &str = "__be3_dylink_";
pub const SDK_FILE_PREFIX: &str = "sdk-";
pub const PLUGIN_TLS: &str = "__be3_plugin_tls";
pub const PLUGIN_TLS_SIZE: u32 = 16 * 1024;
pub const TABLE: &str = "__indirect_function_table";
pub const STACK_POINTER: &str = "__stack_pointer";
pub const MEMORY_BASE: &str = "__memory_base";
pub const TABLE_BASE: &str = "__table_base";
pub const TLS_BASE: &str = "__tls_base";
pub const TLS_SIZE: &str = "__tls_size";
pub const MALLOC: &str = "malloc";
pub const APPLY_DATA_RELOCS: &str = "__wasm_apply_data_relocs";
pub const ENV: &str = "env";
pub const GOT_MEMORY: &str = "GOT.mem";
pub const GOT_FUNCTION: &str = "GOT.func";
pub const GOT_THREAD_LOCAL: &str = "GOT.tls";
pub const START_ARGUMENT_TLS: u32 = 4;
const MAX_ALIGN_LOG2: u32 = 16;
const IDENTITY_LENGTH: usize = 64;

pub fn sdk_identity<'a>(
    exports: impl IntoIterator<Item = &'a str>,
) -> Option<Result<String, String>> {
    exports
        .into_iter()
        .find_map(|export| export.strip_prefix(SDK_EXPORT_PREFIX))
        .map(|identity| {
            let valid = identity.len() == IDENTITY_LENGTH
                && identity
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
            valid.then(|| identity.to_owned()).ok_or_else(|| {
                format!("the plugin names {identity:?} as its plugin SDK, which is not a SHA-256")
            })
        })
}

pub fn sdk_file_name(identity: &str) -> String {
    format!("{SDK_FILE_PREFIX}{identity}.wasm")
}

pub fn check_sdk(bytes: &[u8], identity: &str) -> Result<(), String> {
    let digest: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if digest == identity {
        Ok(())
    } else {
        Err(format!(
            "its SHA-256 is {digest}, not the {identity} it is named by"
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declared {
    pub memory_size: u32,
    pub memory_align: u32,
    pub table_size: u32,
    pub table_align: u32,
}

impl Declared {
    pub fn of<'a>(exports: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let layout = exports
            .into_iter()
            .find_map(|export| export.strip_prefix(LAYOUT_EXPORT_PREFIX))
            .ok_or("the plugin does not say how much memory and table it needs")?;
        let unreadable = || format!("the plugin's layout {layout} is not one this reads");
        let numbers: Vec<u32> = layout
            .split('_')
            .map(|number| number.parse::<u32>())
            .collect::<Result<_, _>>()
            .map_err(|_| unreadable())?;
        let [memory_size, memory_align, table_size, table_align] = numbers[..] else {
            return Err(unreadable());
        };
        Ok(Self {
            memory_size,
            memory_align: memory_align.min(MAX_ALIGN_LOG2),
            table_size,
            table_align: table_align.min(MAX_ALIGN_LOG2),
        })
    }

    pub fn allocation(&self) -> Result<u32, String> {
        self.memory_size
            .checked_add(1 << self.memory_align)
            .ok_or_else(too_big)
    }
}

#[derive(Default)]
pub struct Sdk {
    pub functions: HashSet<String>,
    pub addresses: HashMap<String, i32>,
}

impl Sdk {
    pub fn reserve(&self) -> Result<u32, String> {
        self.addresses
            .get(PLUGIN_TLS)
            .map(|reserve| *reserve as u32)
            .ok_or_else(|| format!("the plugin SDK does not export {PLUGIN_TLS}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum Import {
    Table,
    StackPointer,
    Constant(i32),
    Slot(i32),
    Address(i32),
    Own,
    Elsewhere,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub memory_base: u32,
    pub memory_size: u32,
    pub table_base: u32,
    pub table_grow: u64,
    pub sdk_slots: Vec<(u64, String)>,
    pub plugin_slots: Vec<(u64, String)>,
    pub imports: Vec<Import>,
    pub reserve: u32,
    pub tls_size: u32,
    pub image: u32,
}

fn too_big() -> String {
    "the plugin asks for more than a 32-bit address space holds".to_owned()
}

fn aligned(value: u32, align_log2: u32) -> Option<u32> {
    value.div_ceil(1 << align_log2).checked_mul(1 << align_log2)
}

pub fn plan<'a>(
    declared: Declared,
    allocated: u32,
    table_size: u64,
    imports: impl IntoIterator<Item = (&'a str, &'a str)>,
    sdk: &Sdk,
) -> Result<Layout, String> {
    let reserve = sdk.reserve()?;
    if allocated == 0 {
        return Err("the plugin SDK could not allocate the plugin's memory".to_owned());
    }
    let memory_base = aligned(allocated, declared.memory_align).ok_or_else(too_big)?;
    let table_base = u32::try_from(table_size)
        .ok()
        .and_then(|size| aligned(size, declared.table_align))
        .ok_or_else(too_big)?;
    let mut next = u64::from(table_base) + u64::from(declared.table_size);
    let mut sdk_slots = Vec::new();
    let mut plugin_slots = Vec::new();
    let mut resolved = Vec::new();
    for (module, name) in imports {
        let import = match (module, name) {
            (ENV, TABLE) => Import::Table,
            (ENV, STACK_POINTER) => Import::StackPointer,
            (ENV, MEMORY_BASE) => Import::Constant(memory_base as i32),
            (ENV, TABLE_BASE) => Import::Constant(table_base as i32),
            (GOT_FUNCTION, _) => {
                let slots = match sdk.functions.contains(name) {
                    true => &mut sdk_slots,
                    false => &mut plugin_slots,
                };
                slots.push((next, name.to_owned()));
                next += 1;
                Import::Slot((next - 1) as i32)
            }
            (GOT_MEMORY, _) => match sdk.addresses.get(name) {
                Some(address) => Import::Address(*address),
                None => Import::Own,
            },
            _ => Import::Elsewhere,
        };
        resolved.push(import);
    }
    Ok(Layout {
        memory_base,
        memory_size: declared.memory_size,
        table_base,
        table_grow: next - table_size,
        sdk_slots,
        plugin_slots,
        imports: resolved,
        reserve,
        tls_size: 0,
        image: 0,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Image {
    pub from: u32,
    pub to: u32,
    pub length: u32,
}

impl Layout {
    pub fn own_address(&self, name: &str, offset: i32) -> Result<u32, String> {
        self.memory_base
            .checked_add_signed(offset)
            .ok_or_else(|| format!("{name} lies past a 32-bit address space"))
    }

    pub fn relocated(&mut self, image: u32, tls_size: u32) {
        self.image = image;
        self.tls_size = tls_size;
    }

    pub fn thread_locals(&self, base: u32) -> Result<Option<Image>, String> {
        let start = self.reserve;
        if self.tls_size <= start {
            return Ok(None);
        }
        let overflow = || "the plugin's thread-locals lie past a 32-bit address space".to_owned();
        Ok(Some(Image {
            from: self.image.checked_add(start).ok_or_else(overflow)?,
            to: base.checked_add(start).ok_or_else(overflow)?,
            length: self.tls_size - start,
        }))
    }
}

pub fn spawned_thread_locals(start_argument: i32) -> Result<u32, String> {
    (start_argument as u32)
        .checked_add(START_ARGUMENT_TLS)
        .ok_or_else(|| {
            "a spawned thread's start argument lies past a 32-bit address space".to_owned()
        })
}

pub fn thread_local_slot(address: u32, own_base: u32) -> i32 {
    address.wrapping_sub(own_base) as i32
}
