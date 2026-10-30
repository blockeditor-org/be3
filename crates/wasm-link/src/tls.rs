use std::collections::{HashMap, HashSet};

use wasm_dylink::GOT_THREAD_LOCAL;

use crate::module::{self, CUSTOM, HEADER, IMPORT, Reader};

const CODE: u8 = 10;
const LINKING: &str = "linking";
const RELOCATIONS: &str = "reloc.";
const CODE_RELOCATIONS: &str = "reloc.CODE";
const SEGMENT_INFO: u8 = 5;
const SYMBOL_TABLE: u8 = 8;
const SEGMENT_THREAD_LOCAL: u64 = 0x2;

const DATA_SYMBOL: u8 = 1;
const GLOBAL_SYMBOL: u8 = 2;
const SECTION_SYMBOL: u8 = 3;
const BINDING_LOCAL: u64 = 0x2;
const UNDEFINED: u64 = 0x10;
const EXPLICIT_NAME: u64 = 0x40;

const GLOBAL_INDEX_LEB: u8 = 7;
const TLS_SLEB: u8 = 21;
const TLS_SLEB64: u8 = 25;
const WITH_ADDEND: [u8; 14] = [3, 4, 5, 8, 9, 11, 14, 15, 16, 17, 21, 22, 23, 25];
const I32_CONST: u8 = 0x41;
const GLOBAL_GET: u8 = 0x23;
const I32: u8 = 0x7f;
const MUTABLE: u8 = 1;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ThreadLocals {
    pub defined: HashSet<String>,
    pub reached: HashSet<String>,
}

struct Symbol<'a> {
    kind: u8,
    flags: u64,
    index: Option<u32>,
    name: Option<String>,
    segment: Option<u32>,
    raw: &'a [u8],
}

impl Symbol<'_> {
    fn undefined(&self) -> bool {
        self.flags & UNDEFINED != 0
    }
}

#[derive(Clone, Copy)]
struct Relocation {
    kind: u8,
    offset: u32,
    symbol: u32,
    addend: i64,
}

fn symbols(table: &[u8]) -> Result<Vec<Symbol<'_>>, String> {
    let mut reader = Reader::new(table);
    let count = reader.u32()?;
    let mut symbols = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let start = reader.at;
        let kind = reader.byte()?;
        let flags = reader.unsigned()?;
        let undefined = flags & UNDEFINED != 0;
        let mut segment = None;
        let (index, name) = match kind {
            DATA_SYMBOL => {
                let name = reader.name()?;
                if !undefined {
                    segment = Some(reader.u32()?);
                    reader.unsigned()?;
                    reader.unsigned()?;
                }
                (None, Some(name))
            }
            SECTION_SYMBOL => (Some(reader.u32()?), None),
            _ => {
                let index = reader.u32()?;
                let named = !undefined || flags & EXPLICIT_NAME != 0;
                (Some(index), if named { Some(reader.name()?) } else { None })
            }
        };
        symbols.push(Symbol {
            kind,
            flags,
            index,
            name,
            segment,
            raw: &table[start..reader.at],
        });
    }
    Ok(symbols)
}

fn relocations(contents: &[u8]) -> Result<(u32, Vec<Relocation>), String> {
    let mut reader = Reader::new(contents);
    let target = reader.u32()?;
    let count = reader.u32()?;
    let mut relocations = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let kind = reader.byte()?;
        let offset = reader.u32()?;
        let symbol = reader.u32()?;
        let addend = match WITH_ADDEND.contains(&kind) {
            true => reader.signed()?,
            false => 0,
        };
        relocations.push(Relocation {
            kind,
            offset,
            symbol,
            addend,
        });
    }
    Ok((target, relocations))
}

fn encode_relocations(target: u32, relocations: &[Relocation]) -> Vec<u8> {
    let mut out = Vec::new();
    module::unsigned(u64::from(target), &mut out);
    module::unsigned(relocations.len() as u64, &mut out);
    for relocation in relocations {
        out.push(relocation.kind);
        module::unsigned(u64::from(relocation.offset), &mut out);
        module::unsigned(u64::from(relocation.symbol), &mut out);
        if WITH_ADDEND.contains(&relocation.kind) {
            module::signed(relocation.addend, &mut out);
        }
    }
    out
}

fn custom_name(contents: &[u8]) -> Result<(String, usize), String> {
    let mut reader = Reader::new(contents);
    let name = reader.name()?;
    Ok((name, reader.at))
}

struct Linking<'a> {
    version: u64,
    subsections: Vec<(u8, &'a [u8])>,
}

fn linking(payload: &[u8]) -> Result<Linking<'_>, String> {
    let mut reader = Reader::new(payload);
    let version = reader.unsigned()?;
    let mut subsections = Vec::new();
    while !reader.done() {
        let kind = reader.byte()?;
        let length = reader.u32()? as usize;
        subsections.push((kind, reader.take(length)?));
    }
    Ok(Linking {
        version,
        subsections,
    })
}

struct Object<'a> {
    sections: Vec<module::Section<'a>>,
    linking: Option<(usize, Linking<'a>)>,
    relocations: Vec<(usize, String, u32, Vec<Relocation>)>,
}

impl<'a> Object<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let sections = module::sections(bytes)?;
        let mut parsed_linking = None;
        let mut parsed_relocations = Vec::new();
        for (at, section) in sections.iter().enumerate() {
            if section.id != CUSTOM {
                continue;
            }
            let (name, start) = custom_name(section.contents)?;
            let payload = &section.contents[start..];
            if name == LINKING {
                parsed_linking = Some((at, linking(payload)?));
            } else if name.starts_with(RELOCATIONS) {
                let (target, entries) = relocations(payload)?;
                parsed_relocations.push((at, name, target, entries));
            }
        }
        Ok(Self {
            sections,
            linking: parsed_linking,
            relocations: parsed_relocations,
        })
    }

    fn subsection(&self, wanted: u8) -> Option<&'a [u8]> {
        let (_, linking) = self.linking.as_ref()?;
        linking
            .subsections
            .iter()
            .find(|(kind, _)| *kind == wanted)
            .map(|(_, payload)| *payload)
    }

    fn symbol_table(&self) -> Result<Vec<Symbol<'a>>, String> {
        self.subsection(SYMBOL_TABLE)
            .map_or(Ok(Vec::new()), symbols)
    }

    fn thread_local_segments(&self) -> Result<HashSet<u32>, String> {
        let mut found = HashSet::new();
        let Some(info) = self.subsection(SEGMENT_INFO) else {
            return Ok(found);
        };
        let mut reader = Reader::new(info);
        let count = reader.u32()?;
        for segment in 0..count {
            reader.name()?;
            reader.unsigned()?;
            if reader.unsigned()? & SEGMENT_THREAD_LOCAL != 0 {
                found.insert(segment);
            }
        }
        Ok(found)
    }
}

fn is_thread_local(kind: u8) -> bool {
    kind == TLS_SLEB || kind == TLS_SLEB64
}

pub fn is_object(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER && &bytes[..4] == b"\0asm"
}

pub fn scan(bytes: &[u8]) -> Result<ThreadLocals, String> {
    let object = Object::parse(bytes)?;
    let symbols = object.symbol_table()?;
    let segments = object.thread_local_segments()?;
    let mut found = ThreadLocals::default();
    for symbol in &symbols {
        if symbol.kind == DATA_SYMBOL
            && symbol.flags & BINDING_LOCAL == 0
            && symbol
                .segment
                .is_some_and(|segment| segments.contains(&segment))
            && let Some(name) = &symbol.name
        {
            found.defined.insert(name.clone());
        }
    }
    for (_, _, _, relocations) in &object.relocations {
        for relocation in relocations {
            if !is_thread_local(relocation.kind) {
                continue;
            }
            let symbol = symbols.get(relocation.symbol as usize).ok_or_else(|| {
                format!(
                    "a relocation names symbol {}, past the table",
                    relocation.symbol
                )
            })?;
            if symbol.undefined()
                && let Some(name) = &symbol.name
            {
                found.reached.insert(name.clone());
            }
        }
    }
    Ok(found)
}

fn imported_globals(bytes: &[u8]) -> Result<u32, String> {
    Ok(module::imports(bytes)?
        .iter()
        .filter(|import| import.kind == module::GLOBAL_KIND)
        .count() as u32)
}

pub fn symbol_name(name: &str) -> String {
    format!("{GOT_THREAD_LOCAL}.{name}")
}

pub fn through_got(bytes: &[u8], foreign: &HashSet<String>) -> Result<Option<Vec<u8>>, String> {
    let object = Object::parse(bytes)?;
    let symbols = object.symbol_table()?;
    let is_foreign = |index: u32| {
        symbols.get(index as usize).is_some_and(|symbol| {
            symbol.kind == DATA_SYMBOL
                && symbol.undefined()
                && symbol
                    .name
                    .as_ref()
                    .is_some_and(|name| foreign.contains(name))
        })
    };
    let mut reached: Vec<String> = Vec::new();
    let mut added: HashMap<String, u32> = HashMap::new();
    let mut code = None;
    for (at, (section, name, target, relocations)) in object.relocations.iter().enumerate() {
        for relocation in relocations {
            if !is_thread_local(relocation.kind) || !is_foreign(relocation.symbol) {
                continue;
            }
            let symbol = symbols[relocation.symbol as usize]
                .name
                .clone()
                .unwrap_or_default();
            if name != CODE_RELOCATIONS {
                return Err(format!(
                    "{name} reaches the thread-local {symbol} of another module, which only code \
                     may do"
                ));
            }
            if relocation.kind == TLS_SLEB64 {
                return Err(format!(
                    "64-bit code reaches the thread-local {symbol} of another module, which is \
                     not supported"
                ));
            }
            if relocation.addend != 0 {
                return Err(format!(
                    "code reaches {} bytes into the thread-local {symbol} of another module, \
                     which is not supported",
                    relocation.addend
                ));
            }
            if !added.contains_key(&symbol) {
                added.insert(symbol.clone(), reached.len() as u32);
                reached.push(symbol);
            }
            code = Some((at, *section, *target));
        }
    }
    let Some((at, relocations_section, target)) = code else {
        return Ok(None);
    };
    if !object.sections.iter().any(|section| section.id == IMPORT) {
        return Err("an object that reaches a thread-local imports nothing".to_owned());
    }
    let reaches =
        |relocation: &Relocation| is_thread_local(relocation.kind) && is_foreign(relocation.symbol);
    let relocations = &object.relocations[at].3;
    let globals = imported_globals(bytes)?;
    let shift = reached.len() as u32;
    let mut out = bytes[..HEADER].to_vec();
    for (index, section) in object.sections.iter().enumerate() {
        let contents: Vec<u8> = if section.id == IMPORT {
            with_imports(section.contents, &reached)?
        } else if section.id == CODE {
            let mut code = section.contents.to_vec();
            for relocation in relocations.iter().filter(|relocation| reaches(relocation)) {
                let opcode = (relocation.offset as usize)
                    .checked_sub(1)
                    .and_then(|at| code.get_mut(at))
                    .ok_or("a thread-local relocation lies outside the code")?;
                if *opcode != I32_CONST {
                    return Err(format!(
                        "a thread-local relocation at {} is not an i32.const",
                        relocation.offset
                    ));
                }
                *opcode = GLOBAL_GET;
            }
            code
        } else if index == relocations_section {
            let rewritten: Vec<Relocation> = relocations
                .iter()
                .map(|relocation| match reaches(relocation) {
                    true => Relocation {
                        kind: GLOBAL_INDEX_LEB,
                        ..*relocation
                    },
                    false => *relocation,
                })
                .collect();
            let mut contents = Vec::new();
            module::name(CODE_RELOCATIONS, &mut contents);
            contents.extend(encode_relocations(target, &rewritten));
            contents
        } else if let Some((_, linking)) = object
            .linking
            .as_ref()
            .filter(|(linking_at, _)| *linking_at == index)
        {
            let mut contents = Vec::new();
            module::name(LINKING, &mut contents);
            module::unsigned(linking.version, &mut contents);
            for (kind, payload) in &linking.subsections {
                let payload = match *kind {
                    SYMBOL_TABLE => with_symbols(&symbols, &added, globals, shift),
                    _ => payload.to_vec(),
                };
                contents.push(*kind);
                module::unsigned(payload.len() as u64, &mut contents);
                contents.extend(payload);
            }
            contents
        } else {
            section.contents.to_vec()
        };
        module::section(section.id, &contents, &mut out);
    }
    Ok(Some(out))
}

fn with_imports(contents: &[u8], reached: &[String]) -> Result<Vec<u8>, String> {
    let mut reader = Reader::new(contents);
    let count = reader.u32()?;
    let mut out = Vec::new();
    module::unsigned(u64::from(count) + reached.len() as u64, &mut out);
    out.extend_from_slice(&contents[reader.at..]);
    for name in reached {
        module::name(GOT_THREAD_LOCAL, &mut out);
        module::name(name, &mut out);
        out.extend_from_slice(&[module::GLOBAL_KIND, I32, MUTABLE]);
    }
    Ok(out)
}

fn with_symbols(
    symbols: &[Symbol<'_>],
    added: &HashMap<String, u32>,
    globals: u32,
    shift: u32,
) -> Vec<u8> {
    let mut out = Vec::new();
    module::unsigned(symbols.len() as u64, &mut out);
    for symbol in symbols {
        let reached = match (symbol.kind, &symbol.name) {
            (DATA_SYMBOL, Some(name)) if symbol.undefined() => added.get(name),
            _ => None,
        };
        match (symbol.kind, symbol.index, &symbol.name, reached) {
            (DATA_SYMBOL, _, Some(name), Some(offset)) => {
                out.push(GLOBAL_SYMBOL);
                module::unsigned(UNDEFINED | EXPLICIT_NAME, &mut out);
                module::unsigned(u64::from(globals + offset), &mut out);
                module::name(&symbol_name(name), &mut out);
            }
            (GLOBAL_SYMBOL, Some(index), Some(name), _)
                if !symbol.undefined() && index >= globals =>
            {
                out.push(GLOBAL_SYMBOL);
                module::unsigned(symbol.flags, &mut out);
                module::unsigned(u64::from(index + shift), &mut out);
                module::name(name, &mut out);
            }
            _ => out.extend_from_slice(symbol.raw),
        }
    }
    out
}
