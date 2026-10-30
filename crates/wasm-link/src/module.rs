pub const CUSTOM: u8 = 0;
pub(crate) const IMPORT: u8 = 2;
const GLOBAL: u8 = 6;
const EXPORT: u8 = 7;
pub(crate) const HEADER: usize = 8;
const ORDER: [u8; 13] = [1, 2, 3, 4, 5, 13, 6, 7, 8, 9, 12, 10, 11];

pub const FUNCTION: u8 = 0;
pub const TABLE: u8 = 1;
pub const MEMORY: u8 = 2;
pub const GLOBAL_KIND: u8 = 3;
pub const TAG: u8 = 4;

#[derive(Clone, Copy)]
pub struct Section<'a> {
    pub id: u8,
    pub contents: &'a [u8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub kind: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryInfo {
    pub memory_size: u32,
    pub memory_align: u32,
    pub table_size: u32,
    pub table_align: u32,
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub(crate) at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }

    pub(crate) fn byte(&mut self) -> Result<u8, String> {
        let byte = *self
            .bytes
            .get(self.at)
            .ok_or("the module ends in the middle of a section")?;
        self.at += 1;
        Ok(byte)
    }

    pub(crate) fn unsigned(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        let mut shift = 0;
        loop {
            let byte = self.byte()?;
            if shift < 64 {
                value |= u64::from(byte & 0x7f) << shift;
            }
            shift += 7;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
    }

    pub(crate) fn signed(&mut self) -> Result<i64, String> {
        let mut value = 0i64;
        let mut shift = 0;
        loop {
            let byte = self.byte()?;
            if shift < 64 {
                value |= i64::from(byte & 0x7f) << shift;
            }
            shift += 7;
            if byte & 0x80 == 0 {
                if shift < 64 && byte & 0x40 != 0 {
                    value |= -1i64 << shift;
                }
                return Ok(value);
            }
        }
    }

    pub(crate) fn u32(&mut self) -> Result<u32, String> {
        u32::try_from(self.unsigned()?).map_err(|_| "a count does not fit in 32 bits".to_owned())
    }

    pub(crate) fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or("the module ends in the middle of a section")?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    pub(crate) fn name(&mut self) -> Result<String, String> {
        let length = self.u32()? as usize;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "a name is not UTF-8".to_owned())
    }

    fn limits(&mut self) -> Result<(), String> {
        let flags = self.byte()?;
        self.unsigned()?;
        if flags & 1 != 0 {
            self.unsigned()?;
        }
        Ok(())
    }
}

pub fn sections(bytes: &[u8]) -> Result<Vec<Section<'_>>, String> {
    if bytes.len() < HEADER || &bytes[..4] != b"\0asm" {
        return Err("not a WebAssembly module".to_owned());
    }
    let mut reader = Reader::new(&bytes[HEADER..]);
    let mut sections = Vec::new();
    while !reader.done() {
        let id = reader.byte()?;
        let length = reader.u32()? as usize;
        sections.push(Section {
            id,
            contents: reader.take(length)?,
        });
    }
    Ok(sections)
}

pub fn custom<'a>(bytes: &'a [u8], wanted: &str) -> Result<Option<&'a [u8]>, String> {
    for section in sections(bytes)? {
        if section.id != CUSTOM {
            continue;
        }
        let mut reader = Reader::new(section.contents);
        if reader.name()? == wanted {
            return Ok(Some(&section.contents[reader.at..]));
        }
    }
    Ok(None)
}

pub fn imports(bytes: &[u8]) -> Result<Vec<Import>, String> {
    let Some(section) = sections(bytes)?
        .into_iter()
        .find(|section| section.id == IMPORT)
    else {
        return Ok(Vec::new());
    };
    let mut reader = Reader::new(section.contents);
    let count = reader.u32()?;
    let mut imports = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let module = reader.name()?;
        let name = reader.name()?;
        let kind = reader.byte()?;
        match kind {
            FUNCTION => {
                reader.u32()?;
            }
            TABLE => {
                reader.byte()?;
                reader.limits()?;
            }
            MEMORY => reader.limits()?,
            GLOBAL_KIND => {
                reader.byte()?;
                reader.byte()?;
            }
            TAG => {
                reader.byte()?;
                reader.u32()?;
            }
            other => return Err(format!("an import of an unknown kind {other}")),
        }
        imports.push(Import { module, name, kind });
    }
    Ok(imports)
}

pub fn exports(bytes: &[u8]) -> Result<Vec<Export>, String> {
    let Some(section) = sections(bytes)?
        .into_iter()
        .find(|section| section.id == EXPORT)
    else {
        return Ok(Vec::new());
    };
    let mut reader = Reader::new(section.contents);
    let count = reader.u32()?;
    let mut exports = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let name = reader.name()?;
        let kind = reader.byte()?;
        reader.u32()?;
        exports.push(Export { name, kind });
    }
    Ok(exports)
}

pub fn memory_info(bytes: &[u8]) -> Result<Option<MemoryInfo>, String> {
    let Some(contents) = custom(bytes, "dylink.0")? else {
        return Ok(None);
    };
    let mut reader = Reader::new(contents);
    while !reader.done() {
        let kind = reader.byte()?;
        let length = reader.u32()? as usize;
        let mut subsection = Reader::new(reader.take(length)?);
        if kind == 1 {
            return Ok(Some(MemoryInfo {
                memory_size: subsection.u32()?,
                memory_align: subsection.u32()?,
                table_size: subsection.u32()?,
                table_align: subsection.u32()?,
            }));
        }
    }
    Ok(None)
}

pub(crate) fn unsigned(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub(crate) fn signed(mut value: i64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        if done {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub(crate) fn name(text: &str, out: &mut Vec<u8>) {
    unsigned(text.len() as u64, out);
    out.extend_from_slice(text.as_bytes());
}

pub(crate) fn section(id: u8, contents: &[u8], out: &mut Vec<u8>) {
    out.push(id);
    unsigned(contents.len() as u64, out);
    out.extend_from_slice(contents);
}

pub fn without_custom(bytes: &[u8], unwanted: impl Fn(&str) -> bool) -> Result<Vec<u8>, String> {
    let mut out = bytes[..HEADER].to_vec();
    for current in sections(bytes)? {
        if current.id == CUSTOM {
            let mut reader = Reader::new(current.contents);
            if unwanted(&reader.name()?) {
                continue;
            }
        }
        section(current.id, current.contents, &mut out);
    }
    Ok(out)
}

pub fn with_custom(bytes: &[u8], wanted: &str, payload: &[u8]) -> Vec<u8> {
    let mut contents = Vec::new();
    name(wanted, &mut contents);
    contents.extend_from_slice(payload);
    let mut out = bytes.to_vec();
    section(CUSTOM, &contents, &mut out);
    out
}

fn rank(id: u8) -> Option<usize> {
    ORDER.iter().position(|known| *known == id)
}

fn entries(contents: Option<&[u8]>) -> Result<(u32, &[u8]), String> {
    let Some(contents) = contents else {
        return Ok((0, &[]));
    };
    let mut reader = Reader::new(contents);
    let count = reader.u32()?;
    Ok((count, &contents[reader.at..]))
}

fn appended(count: u32, existing: &[u8], added: u32, entries: &[u8]) -> Vec<u8> {
    let mut contents = Vec::new();
    unsigned(u64::from(count + added), &mut contents);
    contents.extend_from_slice(existing);
    contents.extend_from_slice(entries);
    contents
}

pub fn with_exported_globals(bytes: &[u8], globals: &[(String, i32)]) -> Result<Vec<u8>, String> {
    let sections = sections(bytes)?;
    let imported = imports(bytes)?
        .iter()
        .filter(|import| import.kind == GLOBAL_KIND)
        .count() as u32;
    let global = sections.iter().find(|section| section.id == GLOBAL);
    let export = sections.iter().find(|section| section.id == EXPORT);
    let (defined, existing_globals) = entries(global.map(|section| section.contents))?;
    let (exported, existing_exports) = entries(export.map(|section| section.contents))?;
    let mut new_globals = Vec::new();
    let mut new_exports = Vec::new();
    for (index, (export_name, value)) in globals.iter().enumerate() {
        new_globals.extend_from_slice(&[0x7f, 0x00, 0x41]);
        signed(i64::from(*value), &mut new_globals);
        new_globals.push(0x0b);
        name(export_name, &mut new_exports);
        new_exports.push(GLOBAL_KIND);
        unsigned(
            u64::from(imported + defined + index as u32),
            &mut new_exports,
        );
    }
    let added = globals.len() as u32;
    let global_contents = appended(defined, existing_globals, added, &new_globals);
    let export_contents = appended(exported, existing_exports, added, &new_exports);
    let mut out = bytes[..HEADER].to_vec();
    let mut placed_global = false;
    let mut placed_export = false;
    for current in &sections {
        let current_rank = rank(current.id);
        if !placed_global && current_rank.is_some_and(|at| at > rank(GLOBAL).unwrap_or(0)) {
            section(GLOBAL, &global_contents, &mut out);
            placed_global = true;
        }
        if !placed_export && current_rank.is_some_and(|at| at > rank(EXPORT).unwrap_or(0)) {
            section(EXPORT, &export_contents, &mut out);
            placed_export = true;
        }
        match current.id {
            GLOBAL => {
                section(GLOBAL, &global_contents, &mut out);
                placed_global = true;
            }
            EXPORT => {
                section(EXPORT, &export_contents, &mut out);
                placed_export = true;
            }
            id => section(id, current.contents, &mut out),
        }
    }
    if !placed_global {
        section(GLOBAL, &global_contents, &mut out);
    }
    if !placed_export {
        section(EXPORT, &export_contents, &mut out);
    }
    Ok(out)
}

fn constant(reader: &mut Reader<'_>) -> Result<Option<i64>, String> {
    let mut value = None;
    loop {
        match reader.byte()? {
            0x0b => return Ok(value),
            0x41 | 0x42 => value = Some(reader.signed()?),
            0x23 | 0xd2 => {
                reader.u32()?;
                value = None;
            }
            0xd0 => {
                reader.byte()?;
                value = None;
            }
            0x43 => {
                reader.take(4)?;
                value = None;
            }
            0x44 => {
                reader.take(8)?;
                value = None;
            }
            0x6a | 0x6b | 0x6c | 0x7c | 0x7d | 0x7e => value = None,
            other => return Err(format!("a global's initializer has the opcode {other:#x}")),
        }
    }
}

pub fn exported_constant(bytes: &[u8], wanted: &str) -> Result<Option<i64>, String> {
    let sections = sections(bytes)?;
    let Some(index) = exported_index(&sections, wanted, GLOBAL_KIND)? else {
        return Ok(None);
    };
    let imported = imports(bytes)?
        .iter()
        .filter(|import| import.kind == GLOBAL_KIND)
        .count() as u32;
    let Some(defined) = index.checked_sub(imported) else {
        return Ok(None);
    };
    let Some(section) = sections.iter().find(|section| section.id == GLOBAL) else {
        return Ok(None);
    };
    let mut reader = Reader::new(section.contents);
    let count = reader.u32()?;
    for at in 0..count {
        reader.byte()?;
        reader.byte()?;
        let value = constant(&mut reader)?;
        if at == defined {
            return Ok(value);
        }
    }
    Ok(None)
}

fn exported_index(sections: &[Section<'_>], wanted: &str, kind: u8) -> Result<Option<u32>, String> {
    let Some(section) = sections.iter().find(|section| section.id == EXPORT) else {
        return Ok(None);
    };
    let mut reader = Reader::new(section.contents);
    let count = reader.u32()?;
    for _ in 0..count {
        let name = reader.name()?;
        let exported = reader.byte()?;
        let index = reader.u32()?;
        if name == wanted && exported == kind {
            return Ok(Some(index));
        }
    }
    Ok(None)
}
