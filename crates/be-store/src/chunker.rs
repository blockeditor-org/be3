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

pub fn split(data: &[u8], config: ChunkerConfig) -> Vec<Range<usize>> {
    if data.is_empty() {
        return Vec::new();
    }
    let mask = config.mask();
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < data.len() {
        let remaining = data.len() - start;
        if remaining <= config.minimum {
            ranges.push(start..data.len());
            break;
        }
        let limit = remaining.min(config.maximum);
        let mut digest = 0u64;
        let mut length = config.minimum;
        while length < limit {
            digest = (digest << 1).wrapping_add(GEAR[usize::from(data[start + length])]);
            length += 1;
            if digest & mask == 0 {
                break;
            }
        }
        ranges.push(start..start + length);
        start += length;
    }
    ranges
}
