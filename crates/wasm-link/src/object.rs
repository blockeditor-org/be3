const DATA: u8 = 11;
const DATA_SYMBOL: u8 = 1;
const SEGMENT_INFO: u8 = 5;
const SYMBOL_TABLE: u8 = 8;
const LINKING_VERSION: u64 = 2;
const SEGMENT_TLS: u64 = 0x2;
const SEGMENT_RETAIN: u64 = 0x4;
pub const SYMBOL_HIDDEN: u64 = 0x4;
const SYMBOL_TLS: u64 = 0x400;
pub const SYMBOL_NO_STRIP: u64 = 0x80;
const FEATURES: [&str; 2] = ["atomics", "bulk-memory"];

pub struct Symbol<'a> {
    pub name: &'a str,
    pub offset: u32,
    pub size: u32,
}

fn unsigned(mut value: u64, out: &mut Vec<u8>) {
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

fn name(text: &str, out: &mut Vec<u8>) {
    unsigned(text.len() as u64, out);
    out.extend_from_slice(text.as_bytes());
}

fn section(id: u8, contents: &[u8], out: &mut Vec<u8>) {
    out.push(id);
    unsigned(contents.len() as u64, out);
    out.extend_from_slice(contents);
}

fn custom(title: &str, payload: &[u8], out: &mut Vec<u8>) {
    let mut contents = Vec::new();
    name(title, &mut contents);
    contents.extend_from_slice(payload);
    section(0, &contents, out);
}

pub fn thread_local_segment(
    segment: &str,
    size: u32,
    align_log2: u32,
    symbols: &[Symbol<'_>],
    symbol_flags: u64,
) -> Vec<u8> {
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    let mut data = Vec::new();
    unsigned(1, &mut data);
    data.push(0);
    data.extend_from_slice(&[0x41, 0x00, 0x0b]);
    unsigned(u64::from(size), &mut data);
    data.resize(data.len() + size as usize, 0);
    section(DATA, &data, &mut out);
    let mut linking = Vec::new();
    unsigned(LINKING_VERSION, &mut linking);
    let mut table = Vec::new();
    unsigned(symbols.len() as u64, &mut table);
    for symbol in symbols {
        table.push(DATA_SYMBOL);
        unsigned(symbol_flags | SYMBOL_TLS, &mut table);
        name(symbol.name, &mut table);
        unsigned(0, &mut table);
        unsigned(u64::from(symbol.offset), &mut table);
        unsigned(u64::from(symbol.size), &mut table);
    }
    linking.push(SYMBOL_TABLE);
    unsigned(table.len() as u64, &mut linking);
    linking.extend_from_slice(&table);
    let mut segments = Vec::new();
    unsigned(1, &mut segments);
    name(segment, &mut segments);
    unsigned(u64::from(align_log2), &mut segments);
    unsigned(SEGMENT_TLS | SEGMENT_RETAIN, &mut segments);
    linking.push(SEGMENT_INFO);
    unsigned(segments.len() as u64, &mut linking);
    linking.extend_from_slice(&segments);
    custom("linking", &linking, &mut out);
    let mut features = Vec::new();
    unsigned(FEATURES.len() as u64, &mut features);
    for feature in FEATURES {
        features.push(b'+');
        name(feature, &mut features);
    }
    custom("target_features", &features, &mut out);
    out
}
