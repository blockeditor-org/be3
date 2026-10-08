use crate::display::DisplayMode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub mode: DisplayMode,
    pub preferred: bool,
    pub interlaced: bool,
}

pub struct Timing {
    pub clock_khz: u32,
    pub htotal: u16,
    pub vtotal: u16,
    pub vscan: u16,
    pub interlaced: bool,
    pub double_scan: bool,
}

pub fn refresh_millihertz(timing: &Timing) -> u32 {
    let (htotal, vtotal) = (u64::from(timing.htotal), u64::from(timing.vtotal));
    if htotal == 0 || vtotal == 0 {
        return 0;
    }
    let mut refresh = (u64::from(timing.clock_khz) * 1_000_000 / htotal + vtotal / 2) / vtotal;
    if timing.interlaced {
        refresh *= 2;
    }
    if timing.double_scan {
        refresh /= 2;
    }
    if timing.vscan > 1 {
        refresh /= u64::from(timing.vscan);
    }
    u32::try_from(refresh).unwrap_or(u32::MAX)
}

fn usable(candidates: &[Candidate]) -> impl Iterator<Item = (usize, &Candidate)> {
    let progressive = candidates.iter().any(|candidate| !candidate.interlaced);
    candidates
        .iter()
        .enumerate()
        .filter(move |(_, candidate)| !(progressive && candidate.interlaced))
}

pub fn listed(candidates: &[Candidate]) -> Vec<DisplayMode> {
    let mut modes: Vec<DisplayMode> = usable(candidates)
        .map(|(_, candidate)| candidate.mode)
        .collect();
    modes.sort_by(|a, b| {
        (b.width, b.height, b.refresh_millihertz).cmp(&(a.width, a.height, a.refresh_millihertz))
    });
    modes.dedup();
    modes
}

pub fn default_mode(candidates: &[Candidate]) -> Option<usize> {
    let size = |candidate: &Candidate| (candidate.mode.width, candidate.mode.height);
    let preferred = usable(candidates)
        .find(|(_, candidate)| candidate.preferred)
        .or_else(|| {
            usable(candidates).max_by_key(|(_, candidate)| {
                u64::from(candidate.mode.width) * u64::from(candidate.mode.height)
            })
        })
        .map(|(_, candidate)| size(candidate))?;
    usable(candidates)
        .filter(|(_, candidate)| size(candidate) == preferred)
        .min_by_key(|(index, candidate)| {
            (
                std::cmp::Reverse(candidate.mode.refresh_millihertz),
                !candidate.preferred,
                *index,
            )
        })
        .map(|(index, _)| index)
}

pub fn choose(candidates: &[Candidate], wanted: Option<DisplayMode>) -> Option<usize> {
    wanted
        .and_then(|wanted| {
            usable(candidates)
                .find(|(_, candidate)| candidate.mode == wanted)
                .map(|(index, _)| index)
        })
        .or_else(|| default_mode(candidates))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub make: String,
    pub model: String,
    pub serial: String,
    pub name: Option<String>,
}

const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];

pub fn identity(edid: &[u8]) -> Option<Identity> {
    if edid.len() < 128 || edid[..8] != HEADER {
        return None;
    }
    let packed = u16::from_be_bytes([edid[8], edid[9]]);
    let letter = |shift: u16| char::from(b'@' + ((packed >> shift) & 0x1f) as u8);
    let make: String = [letter(10), letter(5), letter(0)].into_iter().collect();
    let product = u16::from_le_bytes([edid[10], edid[11]]);
    let number = u32::from_le_bytes([edid[12], edid[13], edid[14], edid[15]]);
    let mut name = None;
    let mut serial_text = None;
    for descriptor in edid[54..126].as_chunks::<18>().0 {
        if descriptor[..3] != [0, 0, 0] {
            continue;
        }
        let text = || {
            let bytes = &descriptor[5..];
            let end = bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(bytes.len());
            let text: String = bytes[..end]
                .iter()
                .map(|byte| match byte {
                    0x20..=0x7e => char::from(*byte),
                    _ => ' ',
                })
                .collect();
            Some(text.trim().to_owned()).filter(|text| !text.is_empty())
        };
        match descriptor[3] {
            0xfc => name = text(),
            0xff => serial_text = text(),
            _ => {}
        }
    }
    let model = name.clone().unwrap_or_else(|| format!("{product:04X}"));
    let serial = serial_text.unwrap_or_else(|| match number {
        0 => String::new(),
        number => number.to_string(),
    });
    Some(Identity {
        make,
        model,
        serial,
        name,
    })
}

pub fn monitor_id(identity: Option<&Identity>, connector: &str) -> String {
    match identity {
        Some(identity) if !identity.serial.is_empty() => {
            format!("{}|{}|{}", identity.make, identity.model, identity.serial)
        }
        _ => connector.to_owned(),
    }
}

pub fn monitor_name(identity: Option<&Identity>, connector: &str) -> String {
    match identity {
        Some(Identity {
            name: Some(name), ..
        }) => name.clone(),
        Some(identity) => format!("{} {}", identity.make, identity.model),
        None => connector.to_owned(),
    }
}

#[cfg(test)]
mod tests;
