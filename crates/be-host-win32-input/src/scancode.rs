#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScanCode {
    pub scan: u16,
    pub extended: bool,
}

impl ScanCode {
    pub const fn new(scan: u16, extended: bool) -> Self {
        Self { scan, extended }
    }
}

const PLAIN: [(u16, u32); 23] = [
    (0x54, 99),
    (0x56, 86),
    (0x57, 87),
    (0x58, 88),
    (0x59, 117),
    (0x64, 183),
    (0x65, 184),
    (0x66, 185),
    (0x67, 186),
    (0x68, 187),
    (0x69, 188),
    (0x6A, 189),
    (0x6B, 190),
    (0x6C, 191),
    (0x6D, 192),
    (0x6E, 193),
    (0x70, 93),
    (0x73, 89),
    (0x76, 194),
    (0x79, 92),
    (0x7B, 94),
    (0x7D, 124),
    (0x7E, 121),
];

const EXTENDED: [(u16, u32); 21] = [
    (0x1C, 96),
    (0x1D, 97),
    (0x35, 98),
    (0x37, 99),
    (0x38, 100),
    (0x47, 102),
    (0x48, 103),
    (0x49, 104),
    (0x4B, 105),
    (0x4D, 106),
    (0x4F, 107),
    (0x50, 108),
    (0x51, 109),
    (0x52, 110),
    (0x53, 111),
    (0x5B, 125),
    (0x5C, 126),
    (0x5D, 127),
    (0x5E, 116),
    (0x5F, 142),
    (0x63, 143),
];

pub const VK_PAUSE: u16 = 0x13;
pub const VK_NUMLOCK: u16 = 0x90;

const BY_VIRTUAL_KEY: [(u16, u32); 22] = [
    (VK_PAUSE, 119),
    (VK_NUMLOCK, 69),
    (0xA6, 158),
    (0xA7, 159),
    (0xA8, 173),
    (0xA9, 128),
    (0xAA, 217),
    (0xAB, 156),
    (0xAC, 172),
    (0xAD, 113),
    (0xAE, 114),
    (0xAF, 115),
    (0xB0, 163),
    (0xB1, 165),
    (0xB2, 166),
    (0xB3, 164),
    (0xB4, 155),
    (0xB5, 226),
    (0xB6, 157),
    (0xB7, 140),
    (0x5F, 142),
    (0x2C, 99),
];

pub fn evdev_of_scan(code: ScanCode) -> Option<u32> {
    let table: &[(u16, u32)] = match code.extended {
        true => &EXTENDED,
        false => &PLAIN,
    };
    if let Some((_, evdev)) = table.iter().find(|(scan, _)| *scan == code.scan) {
        return Some(*evdev);
    }
    match code.extended {
        false if (0x01..=0x53).contains(&code.scan) => Some(u32::from(code.scan)),
        true if code.scan == 0x36 => Some(54),
        _ => None,
    }
}

pub fn scan_of_evdev(evdev: u32) -> Option<ScanCode> {
    if let Some((scan, _)) = EXTENDED.iter().find(|(_, code)| *code == evdev) {
        return Some(ScanCode::new(*scan, true));
    }
    if let Some((scan, _)) = PLAIN.iter().find(|(_, code)| *code == evdev) {
        return Some(ScanCode::new(*scan, false));
    }
    match evdev {
        69 => None,
        1..=0x53 => Some(ScanCode::new(evdev as u16, false)),
        _ => None,
    }
}

pub fn evdev_of_virtual_key(vk: u16) -> Option<u32> {
    BY_VIRTUAL_KEY
        .iter()
        .find(|(key, _)| *key == vk)
        .map(|(_, evdev)| *evdev)
}

pub fn virtual_key_of_evdev(evdev: u32) -> Option<u16> {
    if scan_of_evdev(evdev).is_some() {
        return None;
    }
    BY_VIRTUAL_KEY
        .iter()
        .find(|(_, code)| *code == evdev)
        .map(|(vk, _)| *vk)
}

pub fn evdev(vk: u16, code: ScanCode) -> Option<u32> {
    match vk {
        VK_PAUSE | VK_NUMLOCK => evdev_of_virtual_key(vk),
        _ => evdev_of_scan(code).or_else(|| evdev_of_virtual_key(vk)),
    }
}

#[cfg(test)]
mod tests;
