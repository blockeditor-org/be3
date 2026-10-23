use be_model::{Document, Model};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;
use crate::presence::{ChannelKind, PresenceKind};

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Terminal {}

impl Root for Terminal {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7465_726d_696e_616c_2d62_6c6f_636b_0001);
}

pub type TerminalContent = Document<Terminal>;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalHost {
    pub machine: String,
    pub user: String,
}

impl PresenceKind for TerminalHost {
    const ID: Uuid = Uuid::from_u128(0x7465_726d_696e_616c_2d68_6f73_7400_0001);
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TerminalMessage {
    Attach(TerminalSize),
    Resize(TerminalSize),
    Input(Vec<u8>),
    Snapshot {
        shell: u64,
        sequence: u64,
        size: TerminalSize,
        screen: Vec<u8>,
    },
    Output {
        shell: u64,
        sequence: u64,
        bytes: Vec<u8>,
    },
    Exited {
        shell: u64,
        status: Option<u32>,
    },
}

impl ChannelKind for TerminalMessage {
    const ID: Uuid = Uuid::from_u128(0x7465_726d_696e_616c_2d63_6861_6e00_0001);
}

pub const LARGEST_OUTPUT: usize = 32 * 1024;

