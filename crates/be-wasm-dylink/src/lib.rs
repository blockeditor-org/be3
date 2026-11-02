#[cfg(test)]
mod tests;

use std::{borrow::Cow, collections::HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LIBRARY_EXPORT_PREFIX: &str = "__be3_lib_";
pub const LAYOUT_EXPORT_PREFIX: &str = "__be3_dylink_";
pub const LIBRARY_FILE_PREFIX: &str = "lib-";
pub const TABLE: &str = "__indirect_function_table";
pub const STACK_POINTER: &str = "__stack_pointer";
pub const MEMORY_BASE: &str = "__memory_base";
pub const TABLE_BASE: &str = "__table_base";
pub const TLS_BASE: &str = "__tls_base";
pub const TLS_SIZE: &str = "__tls_size";
pub const TLS_ALIGN: &str = "__tls_align";
pub const INIT_TLS: &str = "__wasm_init_tls";
pub const APPLY_DATA_RELOCS: &str = "__wasm_apply_data_relocs";
pub const CONSTRUCTORS: &str = "__wasm_call_ctors";
pub const PAGE_SIZE: u32 = 65536;
pub const ENV: &str = "env";
pub const GOT_MEMORY: &str = "GOT.mem";
pub const GOT_FUNCTION: &str = "GOT.func";
pub const GOT_THREAD_LOCAL: &str = "GOT.tls";
pub const START_ARGUMENT_TLS: u32 = 4;
pub const REGION_HEADER: u32 = 8;
const MAX_ALIGN_LOG2: u32 = 16;
const IDENTITY_LENGTH: usize = 64;
const BLOCKS_PER_GROWTH: u32 = 16;
pub const SHORT_SYMBOL_PREFIX: char = '#';
const SHORTENED: [&str; 3] = ["_R", "_Z", "anon."];
const SHORT_SYMBOL_BYTES: usize = 12;
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn identity(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn short_symbol(symbol: &str) -> Cow<'_, str> {
    let short_length = 1 + SHORT_SYMBOL_BYTES * 4 / 3;
    if symbol.len() <= short_length || !SHORTENED.iter().any(|prefix| symbol.starts_with(prefix)) {
        return Cow::Borrowed(symbol);
    }
    let digest = Sha256::digest(symbol.as_bytes());
    let mut short = String::with_capacity(short_length);
    short.push(SHORT_SYMBOL_PREFIX);
    for chunk in digest[..SHORT_SYMBOL_BYTES].chunks(3) {
        let bits = u32::from(chunk[0]) << 16 | u32::from(chunk[1]) << 8 | u32::from(chunk[2]);
        for shift in [18, 12, 6, 0] {
            short.push(char::from(BASE64[(bits >> shift & 63) as usize]));
        }
    }
    Cow::Owned(short)
}

fn is_identity(identity: &str) -> bool {
    identity.len() == IDENTITY_LENGTH
        && identity
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn library_export(position: usize, identity: &str) -> String {
    format!("{LIBRARY_EXPORT_PREFIX}{position}_{identity}")
}

pub fn libraries<'a>(exports: impl IntoIterator<Item = &'a str>) -> Result<Vec<String>, String> {
    let mut named = Vec::new();
    for export in exports {
        let Some(library) = export.strip_prefix(LIBRARY_EXPORT_PREFIX) else {
            continue;
        };
        let unreadable = || {
            format!(
                "the module names {export:?} as a library it needs, which is not \
                 {LIBRARY_EXPORT_PREFIX}<position>_<SHA-256>"
            )
        };
        let (position, identity) = library.split_once('_').ok_or_else(unreadable)?;
        let position: usize = position.parse().map_err(|_| unreadable())?;
        if !is_identity(identity) {
            return Err(unreadable());
        }
        named.push((position, identity.to_owned()));
    }
    named.sort();
    let positions: Vec<usize> = named.iter().map(|(position, _)| *position).collect();
    if positions
        .iter()
        .enumerate()
        .any(|(at, position)| at != *position)
    {
        return Err(format!(
            "the module numbers its libraries {positions:?}, not from 0 up"
        ));
    }
    Ok(named.into_iter().map(|(_, identity)| identity).collect())
}

pub fn library_file_name(identity: &str) -> String {
    format!("{LIBRARY_FILE_PREFIX}{identity}.wasm")
}

pub fn check_library(bytes: &[u8], identity: &str) -> Result<(), String> {
    let digest = self::identity(bytes);
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
    pub tls_size: u32,
    pub tls_align: u32,
}

impl Declared {
    pub fn export(&self) -> String {
        format!(
            "{LAYOUT_EXPORT_PREFIX}{}_{}_{}_{}_{}_{}",
            self.memory_size,
            self.memory_align,
            self.table_size,
            self.table_align,
            self.tls_size,
            self.tls_align
        )
    }

    pub fn of<'a>(exports: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let layout = exports
            .into_iter()
            .find_map(|export| export.strip_prefix(LAYOUT_EXPORT_PREFIX))
            .ok_or("it does not say how much memory, table and thread-local storage it needs")?;
        let unreadable = || format!("its layout {layout} is not one this reads");
        let numbers: Vec<u32> = layout
            .split('_')
            .map(|number| number.parse::<u32>())
            .collect::<Result<_, _>>()
            .map_err(|_| unreadable())?;
        let [
            memory_size,
            memory_align,
            table_size,
            table_align,
            tls_size,
            tls_align,
        ] = numbers[..]
        else {
            return Err(unreadable());
        };
        Ok(Self {
            memory_size,
            memory_align: memory_align.min(MAX_ALIGN_LOG2),
            table_size,
            table_align: table_align.min(MAX_ALIGN_LOG2),
            tls_size,
            tls_align: tls_align.clamp(1, 1 << MAX_ALIGN_LOG2),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Described {
    pub name: String,
    pub imports: Vec<(String, String)>,
    pub functions: HashSet<String>,
    pub globals: HashSet<String>,
    pub exports: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Import {
    Table,
    StackPointer,
    Constant(i32),
    Function(usize),
    Address(usize),
    FunctionSlot(i32),
    ThreadLocal(usize),
    Elsewhere,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    pub memory_base: u32,
    pub table_base: u32,
    pub block: u32,
    pub imports: Vec<Import>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub slot: u64,
    pub module: usize,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocks {
    pub size: u32,
    pub align: u32,
    pub pages: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub modules: Vec<Placed>,
    pub table_grow: u64,
    pub slots: Vec<Slot>,
    pub blocks: Blocks,
    pub region: u32,
}

fn too_big() -> String {
    "the modules ask for more than a 32-bit address space holds".to_owned()
}

fn aligned(value: u64, align: u64) -> u64 {
    value.div_ceil(align.max(1)) * align.max(1)
}

fn declared(modules: &[Described]) -> Result<Vec<Declared>, String> {
    modules
        .iter()
        .skip(1)
        .map(|module| {
            Declared::of(module.exports.iter().map(String::as_str))
                .map_err(|error| format!("{} cannot be linked: {error}", module.name))
        })
        .collect()
}

fn images(declared: &[Declared]) -> (Vec<u64>, u64) {
    let mut next = 0;
    let mut bases = Vec::with_capacity(declared.len());
    for module in declared {
        let base = aligned(next, 1 << module.memory_align);
        bases.push(base);
        next = base + u64::from(module.memory_size);
    }
    (bases, aligned(next, 8) + u64::from(REGION_HEADER))
}

pub fn pages(modules: &[Described]) -> Result<u32, String> {
    let (_, end) = images(&declared(modules)?);
    u32::try_from(end.div_ceil(u64::from(PAGE_SIZE))).map_err(|_| too_big())
}

fn defined(modules: &[Described], below: usize, has: impl Fn(&Described) -> bool) -> Option<usize> {
    modules[..below].iter().position(has)
}

pub fn plan(modules: &[Described], placed: u32, table_size: u64) -> Result<Plan, String> {
    if modules.is_empty() {
        return Err("there is no module to link".to_owned());
    }
    let declared = declared(modules)?;
    let (bases, end) = images(&declared);
    let address = |offset: u64| u32::try_from(u64::from(placed) + offset).map_err(|_| too_big());
    address(end)?;
    let region = address(end - u64::from(REGION_HEADER))?;
    let mut next = table_size;
    let mut table_bases = Vec::with_capacity(declared.len());
    for module in &declared {
        let base = aligned(next, 1 << module.table_align);
        table_bases.push(base);
        next = base + u64::from(module.table_size);
    }
    let mut size = 0;
    let mut align = 1;
    let mut block_offsets = Vec::with_capacity(declared.len());
    for module in &declared {
        let offset = aligned(size, u64::from(module.tls_align));
        block_offsets.push(offset);
        size = offset + u64::from(module.tls_size);
        align = align.max(module.tls_align);
    }
    let align = align.max(4);
    let size = u32::try_from(aligned(size, u64::from(align))).map_err(|_| too_big())?;
    let blocks = Blocks {
        size,
        align,
        pages: u32::try_from(
            (u64::from(size) * u64::from(BLOCKS_PER_GROWTH)).div_ceil(u64::from(PAGE_SIZE)),
        )
        .map_err(|_| too_big())?,
    };
    let mut placed_modules = vec![Placed {
        memory_base: 0,
        table_base: 0,
        block: 0,
        imports: Vec::new(),
    }];
    let mut slots = Vec::new();
    for (index, module) in modules.iter().enumerate().skip(1) {
        let memory_base = address(bases[index - 1])?;
        let table_base = u32::try_from(table_bases[index - 1]).map_err(|_| too_big())?;
        let missing = |from: &str, name: &str, what: &str| {
            format!(
                "{} imports {from}.{name}, which {what} exports",
                module.name
            )
        };
        let mut imports = Vec::with_capacity(module.imports.len());
        for (from, name) in &module.imports {
            let (from, name) = (from.as_str(), name.as_str());
            let import = match (from, name) {
                (ENV, TABLE) => Import::Table,
                (ENV, STACK_POINTER) => Import::StackPointer,
                (ENV, MEMORY_BASE) => Import::Constant(memory_base as i32),
                (ENV, TABLE_BASE) => Import::Constant(table_base as i32),
                (ENV, "memory") => Import::Elsewhere,
                (ENV, _) => Import::Function(
                    defined(modules, index, |module| module.functions.contains(name))
                        .ok_or_else(|| missing(from, name, "no library under it"))?,
                ),
                (GOT_FUNCTION, _) => {
                    let module =
                        defined(modules, index + 1, |module| module.functions.contains(name))
                            .ok_or_else(|| missing(from, name, "nothing it is linked with"))?;
                    slots.push(Slot {
                        slot: next,
                        module,
                        name: name.to_owned(),
                    });
                    next += 1;
                    Import::FunctionSlot((next - 1) as i32)
                }
                (GOT_MEMORY, _) => Import::Address(
                    defined(modules, index + 1, |module| module.globals.contains(name))
                        .ok_or_else(|| missing(from, name, "nothing it is linked with"))?,
                ),
                (GOT_THREAD_LOCAL, _) => Import::ThreadLocal(
                    defined(modules, index, |module| module.globals.contains(name))
                        .ok_or_else(|| missing(from, name, "no library under it"))?,
                ),
                _ => Import::Elsewhere,
            };
            imports.push(import);
        }
        placed_modules.push(Placed {
            memory_base,
            table_base,
            block: u32::try_from(block_offsets[index - 1]).map_err(|_| too_big())?,
            imports,
        });
    }
    Ok(Plan {
        modules: placed_modules,
        table_grow: next - table_size,
        slots,
        blocks,
        region,
    })
}

impl Blocks {
    pub fn carve(&self, start: u32, pages: u32) -> Vec<u32> {
        if self.size == 0 {
            return Vec::new();
        }
        let size = u64::from(self.size);
        let end =
            (u64::from(start) + u64::from(pages) * u64::from(PAGE_SIZE)).min(u64::from(u32::MAX));
        let mut at = aligned(u64::from(start), u64::from(self.align));
        let mut carved = Vec::new();
        while at + size <= end {
            carved.push(at as u32);
            at += size;
        }
        carved
    }
}

impl Plan {
    pub fn address(&self, module: usize, value: i32) -> Result<u32, String> {
        self.modules[module]
            .memory_base
            .checked_add_signed(value)
            .ok_or_else(|| "an address lies past a 32-bit address space".to_owned())
    }

    pub fn block(&self, blocks: u32, module: usize) -> u32 {
        blocks.wrapping_add(self.modules[module].block)
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
