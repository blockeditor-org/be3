use be_block_model::{Document, Edit, Map, Model, ObjectId};
use uuid::Uuid;

use be_block_content::Root;

/// Cells along each side of the world.
pub const SIDE: u16 = 6;
/// Cells in the world, numbered row by row from the top left.
pub const CELLS: u16 = SIDE * SIDE;
/// Gems a new world hides.
pub const GEMS: u16 = 6;

/// A gem hunt: a small world of cells, a few of which hide a gem, and the first hosted block
/// type (guides/blocks/hosted.md). Where the gems lie is the world's secret. The block holds it, so only those who may read
/// the block (its hosts) know it; players see only what the host tells them of the cells they
/// dig: a gem, or how many gems lie next to it.
#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Hunt {
    /// The cells that hide a gem.
    pub gems: Vec<u16>,
    /// Each cell dug, by the account that dug it.
    pub dug: Map<u16, Uuid>,
}

impl Hunt {
    /// A new world whose gems are scattered by `seed`.
    pub fn scattered(seed: u64) -> Self {
        Self {
            gems: scatter(seed),
            dug: Map::default(),
        }
    }

    /// The practice world: its gems always lie in the same cells, the top left corner among
    /// them, so a hunt learnt once (or a test) plays the same way every time.
    pub fn practice() -> Self {
        Self {
            gems: vec![0, 9, 20, 27, 31, 35],
            dug: Map::default(),
        }
    }

    /// Hides the gems again, scattered by `seed`, and fills every dug cell in.
    pub fn new_world(&self, seed: u64) -> Edit {
        let mut changes = vec![Self::GEMS.set(ObjectId::ROOT, &scatter(seed))];
        changes.extend(
            self.dug
                .keys()
                .map(|cell| Self::DUG.put(ObjectId::ROOT, cell, None)),
        );
        changes.into_iter().collect()
    }

    /// `account` digs `cell`. Whether it may is the host's to judge (`may_dig`).
    pub fn dig(cell: u16, account: Uuid) -> Edit {
        Self::DUG.put(ObjectId::ROOT, &cell, Some(&account)).into()
    }

    /// Whether `cell` may be dug now: it is in the world and not dug yet, and some gem is
    /// still hidden.
    pub fn may_dig(&self, cell: u16) -> bool {
        cell < CELLS && !self.dug.contains_key(&cell) && !self.over()
    }

    pub fn has_gem(&self, cell: u16) -> bool {
        self.gems.contains(&cell)
    }

    /// How many gems lie in the cells around `cell`, diagonals included.
    pub fn near(&self, cell: u16) -> u8 {
        neighbours(cell).filter(|&next| self.has_gem(next)).count() as u8
    }

    /// The gems dug up so far.
    pub fn found(&self) -> usize {
        self.dug.keys().filter(|&&cell| self.has_gem(cell)).count()
    }

    /// The gems `account` dug up.
    pub fn found_by(&self, account: Uuid) -> usize {
        self.dug
            .iter()
            .filter(|&(&cell, &by)| by == account && self.has_gem(cell))
            .count()
    }

    /// Whether every gem is dug up.
    pub fn over(&self) -> bool {
        !self.gems.is_empty() && self.found() == self.gems.len()
    }
}

impl Root for Hunt {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6765_6d2d_6875_6e74_2d62_6c6f_636b_0001);
}

pub type HuntContent = Document<Hunt>;

/// The cells around `cell`, diagonals included, that are in the world.
fn neighbours(cell: u16) -> impl Iterator<Item = u16> {
    let (row, column) = ((cell / SIDE) as i32, (cell % SIDE) as i32);
    (-1..=1)
        .flat_map(move |down| (-1..=1).map(move |across| (row + down, column + across)))
        .filter(move |&(next_row, next_column)| {
            (next_row, next_column) != (row, column)
                && (0..SIDE as i32).contains(&next_row)
                && (0..SIDE as i32).contains(&next_column)
        })
        .map(|(next_row, next_column)| next_row as u16 * SIDE + next_column as u16)
}

/// `GEMS` distinct cells, picked by `seed` (a splitmix64 stream), in order.
fn scatter(seed: u64) -> Vec<u16> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    };
    let mut gems = Vec::new();
    while gems.len() < GEMS as usize {
        let cell = (next() % u64::from(CELLS)) as u16;
        if !gems.contains(&cell) {
            gems.push(cell);
        }
    }
    gems.sort_unstable();
    gems
}
