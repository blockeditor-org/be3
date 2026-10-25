pub const MARK: [u8; 4] = [0xff; 4];

const CALL: u8 = 1;

const RETURNED: u8 = 2;

pub const ALLOC: &str = "plugin_alloc";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call<'a> {
    pub name: &'a str,
    pub parent: Option<u64>,
    pub answered: bool,
    pub args: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item<'a> {
    Frame(&'a [u8]),
    Call(Call<'a>),
    Returned(&'a [u8]),
}

const PARENT: u8 = 1;

const ANSWERED: u8 = 2;

pub fn call(name: &str, parent: Option<u64>, answered: bool, args: &[u8]) -> Vec<u8> {
    let mut item = Vec::with_capacity(4 + 1 + 9 + 2 + name.len() + args.len());
    item.extend_from_slice(&MARK);
    item.push(CALL);
    item.push(parent.map_or(0, |_| PARENT) | if answered { ANSWERED } else { 0 });
    item.extend_from_slice(&parent.unwrap_or_default().to_be_bytes());
    item.extend_from_slice(&(name.len() as u16).to_be_bytes());
    item.extend_from_slice(name.as_bytes());
    item.extend_from_slice(args);
    item
}

pub fn returned(result: &[u8]) -> Vec<u8> {
    let mut item = Vec::with_capacity(5 + result.len());
    item.extend_from_slice(&MARK);
    item.push(RETURNED);
    item.extend_from_slice(result);
    item
}

pub fn decode(item: &[u8]) -> Option<Item<'_>> {
    let Some(rest) = item.strip_prefix(&MARK) else {
        return Some(Item::Frame(item));
    };
    let (&kind, rest) = rest.split_first()?;
    match kind {
        CALL => {
            let (&flags, rest) = rest.split_first()?;
            let (parent, rest) = rest.split_first_chunk::<8>()?;
            let (length, rest) = rest.split_first_chunk::<2>()?;
            let length = u16::from_be_bytes(*length) as usize;
            if rest.len() < length {
                return None;
            }
            let (name, args) = rest.split_at(length);
            Some(Item::Call(Call {
                name: std::str::from_utf8(name).ok()?,
                parent: (flags & PARENT != 0).then_some(u64::from_be_bytes(*parent)),
                answered: flags & ANSWERED != 0,
                args,
            }))
        }
        RETURNED => Some(Item::Returned(rest)),
        _ => None,
    }
}

pub fn pack(pointer: u32, length: u32) -> i64 {
    ((pointer as i64) << 32) | length as i64
}

pub fn unpack(packed: i64) -> (u32, u32) {
    ((packed >> 32) as u32, packed as u32)
}
