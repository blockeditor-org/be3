use std::ops::Range;

const GEAR: [u64; 256] = {
    let mut table = [0u64; 256];
    let mut state = 0x243f_6a88_85a3_08d3u64;
    let mut index = 0;
    while index < 256 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        table[index] = state ^ (state >> 31);
        index += 1;
    }
    table
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkerConfig {
    pub minimum: usize,
    pub average: usize,
    pub maximum: usize,
}

impl ChunkerConfig {
    pub const DEFAULT: Self = Self {
        minimum: 256 * 1024,
        average: 1024 * 1024,
        maximum: 4 * 1024 * 1024,
    };

    pub const SMALL: Self = Self {
        minimum: 64,
        average: 256,
        maximum: 1024,
    };

    fn mask(self) -> u64 {
        let bits = self.average.max(2).ilog2();
        (1u64 << bits) - 1
    }
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The chunks of `data`, as ranges. A chunk's end depends only on its own bytes (the digest starts
/// again with each chunk), so a `Splitter` fed the same bytes in pieces cuts the same chunks.
pub fn split(data: &[u8], config: ChunkerConfig) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < data.len() {
        let length = cut(&data[start..], config, &mut Scan::default(), true)
            .expect("the end of the bytes ends a chunk");
        ranges.push(start..start + length);
        start += length;
    }
    ranges
}

/// How far a chunk's scan for its end got, and its digest there.
#[derive(Clone, Copy, Debug, Default)]
struct Scan {
    digest: u64,
    length: usize,
}

/// The length of the chunk at the start of `data`, once `data` decides it: always when `data`
/// is the end of the bytes (`end`), and otherwise when the chunk ends at a boundary inside it or
/// `data` holds `maximum` bytes. `scan` carries the scan on to a longer `data` after a None.
fn cut(data: &[u8], config: ChunkerConfig, scan: &mut Scan, end: bool) -> Option<usize> {
    if data.is_empty() {
        return None;
    }
    if data.len() <= config.minimum {
        return end.then_some(data.len());
    }
    let mask = config.mask();
    scan.length = scan.length.max(config.minimum);
    let limit = data.len().min(config.maximum);
    while scan.length < limit {
        scan.digest = (scan.digest << 1).wrapping_add(GEAR[usize::from(data[scan.length])]);
        scan.length += 1;
        if scan.digest & mask == 0 {
            return Some(scan.length);
        }
    }
    match scan.length >= config.maximum || end {
        true => Some(scan.length),
        false => None,
    }
}

/// Cuts bytes handed over a piece at a time into the chunks `split` would cut them into, holding
/// at most `maximum` bytes: those of the chunk not ended yet.
#[derive(Clone, Debug)]
pub struct Splitter {
    config: ChunkerConfig,
    held: Vec<u8>,
    scan: Scan,
}

impl Splitter {
    pub fn new(config: ChunkerConfig) -> Self {
        Self {
            config,
            held: Vec::new(),
            scan: Scan::default(),
        }
    }

    /// The bytes held for the chunk not ended yet.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Takes `bytes`, handing `chunk` each chunk they end, in order.
    pub fn push<E>(
        &mut self,
        mut bytes: &[u8],
        mut chunk: impl FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<(), E> {
        while !bytes.is_empty() {
            let room = self.config.maximum.max(1) - self.held.len();
            let (taken, rest) = bytes.split_at(room.min(bytes.len()));
            self.held.extend_from_slice(taken);
            debug_assert!(self.held.len() <= self.config.maximum.max(1));
            bytes = rest;
            while let Some(length) = cut(&self.held, self.config, &mut self.scan, false) {
                chunk(&self.held[..length])?;
                self.held.drain(..length);
                self.scan = Scan::default();
            }
        }
        Ok(())
    }

    /// Hands `chunk` the chunks of what is held: the end of the bytes.
    pub fn finish<E>(mut self, mut chunk: impl FnMut(&[u8]) -> Result<(), E>) -> Result<(), E> {
        let mut start = 0;
        while let Some(length) = cut(&self.held[start..], self.config, &mut self.scan, true) {
            chunk(&self.held[start..start + length])?;
            start += length;
            self.scan = Scan::default();
        }
        Ok(())
    }
}
