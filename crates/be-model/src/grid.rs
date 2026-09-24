use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use crate::{Change, FieldRef, Object, ObjectId, Tree, Value, field::Field};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

impl Bounds {
    pub const fn new(left: i32, top: i32, width: u32, height: u32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some()
    }

    pub fn index(self, x: i32, y: i32) -> Option<usize> {
        let column = u32::try_from(i64::from(x) - i64::from(self.left)).ok()?;
        let row = u32::try_from(i64::from(y) - i64::from(self.top)).ok()?;
        (column < self.width && row < self.height)
            .then(|| row as usize * self.width as usize + column as usize)
    }

    pub fn area(self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn points(self) -> impl Iterator<Item = (i32, i32)> {
        (0..self.height).flat_map(move |row| {
            (0..self.width).map(move |column| {
                (
                    self.left.saturating_add_unsigned(column),
                    self.top.saturating_add_unsigned(row),
                )
            })
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Cells {
    size: u8,
    bounds: Bounds,
    bytes: Vec<u8>,
}

impl Cells {
    pub(crate) fn blank(size: u8, bounds: Bounds) -> Self {
        Self {
            size,
            bounds,
            bytes: vec![0; bounds.area() * usize::from(size)],
        }
    }

    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn get(&self, x: i32, y: i32) -> Option<&[u8]> {
        let size = usize::from(self.size);
        let at = self.bounds.index(x, y)? * size;
        self.bytes.get(at..at + size)
    }

    fn get_or_blank(&self, x: i32, y: i32) -> Vec<u8> {
        self.get(x, y)
            .map_or_else(|| vec![0; usize::from(self.size)], <[u8]>::to_vec)
    }

    fn set(&mut self, x: i32, y: i32, value: &[u8]) -> bool {
        let size = usize::from(self.size);
        let Some(index) = self.bounds.index(x, y) else {
            return false;
        };
        if value.len() != size {
            return false;
        }
        let held = &mut self.bytes[index * size..index * size + size];
        if held == value {
            return false;
        }
        held.copy_from_slice(value);
        true
    }

    pub(crate) fn paint(&mut self, cells: &[Paint]) -> bool {
        let mut changed = false;
        for cell in cells {
            if let Some(expected) = &cell.expected
                && self.get(cell.x, cell.y) != Some(expected.as_slice())
            {
                continue;
            }
            changed |= self.set(cell.x, cell.y, &cell.value);
        }
        changed
    }

    pub(crate) fn reshape(&mut self, bounds: Bounds) -> bool {
        if bounds == self.bounds {
            return false;
        }
        let mut reshaped = Self::blank(self.size, bounds);
        for (x, y) in bounds.points() {
            if let Some(value) = self.get(x, y) {
                reshaped.set(x, y, value);
            }
        }
        *self = reshaped;
        true
    }

    pub(crate) fn inverse_paint(&self, cells: &[Paint]) -> (Vec<Paint>, Vec<Paint>) {
        let mut scratch = self.clone();
        let mut back = Vec::new();
        let mut forward = Vec::new();
        for cell in cells {
            let Some(held) = scratch.get(cell.x, cell.y).map(<[u8]>::to_vec) else {
                continue;
            };
            if cell
                .expected
                .as_ref()
                .is_some_and(|expected| *expected != held)
                || held == cell.value
                || cell.value.len() != held.len()
            {
                continue;
            }
            back.push(Paint {
                x: cell.x,
                y: cell.y,
                expected: Some(cell.value.clone()),
                value: held.clone(),
            });
            forward.push(Paint {
                x: cell.x,
                y: cell.y,
                expected: Some(held),
                value: cell.value.clone(),
            });
            scratch.set(cell.x, cell.y, &cell.value);
        }
        (back, forward)
    }

    pub(crate) fn outside(&self, bounds: Bounds) -> Vec<Paint> {
        let blank = vec![0; usize::from(self.size)];
        self.bounds
            .points()
            .filter(|(x, y)| !bounds.contains(*x, *y))
            .filter_map(|(x, y)| {
                let value = self.get(x, y)?;
                (value != blank.as_slice()).then(|| Paint {
                    x,
                    y,
                    expected: Some(blank.clone()),
                    value: value.to_vec(),
                })
            })
            .collect()
    }

    pub(crate) fn merge(base: &Self, ours: &Self, theirs: &Self, conflicts: &mut usize) -> Self {
        let bounds = if ours.bounds == base.bounds {
            theirs.bounds
        } else if theirs.bounds == base.bounds || theirs.bounds == ours.bounds {
            ours.bounds
        } else {
            *conflicts += 1;
            ours.bounds
        };
        let mut merged = Self::blank(ours.size, bounds);
        for (x, y) in bounds.points() {
            let (before, mine, other) = (
                base.get_or_blank(x, y),
                ours.get_or_blank(x, y),
                theirs.get_or_blank(x, y),
            );
            let value = if mine == before {
                other
            } else if other == before || other == mine {
                mine
            } else {
                *conflicts += 1;
                mine
            };
            merged.set(x, y, &value);
        }
        merged
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Paint {
    pub x: i32,
    pub y: i32,
    pub expected: Option<Vec<u8>>,
    pub value: Vec<u8>,
}

pub trait Cell: Copy + PartialEq {
    const SIZE: u8;

    fn to_bytes(self) -> Vec<u8>;

    fn from_bytes(bytes: &[u8]) -> Self;
}

impl<const N: usize> Cell for [u8; N] {
    const SIZE: u8 = N as u8;

    fn to_bytes(self) -> Vec<u8> {
        self.to_vec()
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        bytes.try_into().unwrap_or([0; N])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grid<T> {
    cells: Cells,
    marker: PhantomData<fn() -> T>,
}

impl<T: Cell> Default for Grid<T> {
    fn default() -> Self {
        Self::new(Bounds::default())
    }
}

impl<T: Cell> Grid<T> {
    pub fn new(bounds: Bounds) -> Self {
        Self {
            cells: Cells::blank(T::SIZE, bounds),
            marker: PhantomData,
        }
    }

    pub fn bounds(&self) -> Bounds {
        self.cells.bounds
    }

    pub fn bytes(&self) -> &[u8] {
        &self.cells.bytes
    }

    pub fn get(&self, x: i32, y: i32) -> Option<T> {
        self.cells.get(x, y).map(T::from_bytes)
    }

    pub fn set(&mut self, x: i32, y: i32, value: T) {
        self.cells.set(x, y, &value.to_bytes());
    }
}

impl<T: Cell> Field for Grid<T> {
    fn blank() -> Value {
        Value::Grid(Cells::blank(T::SIZE, Bounds::default()))
    }

    fn read(_tree: &Tree, value: &Value) -> Self {
        match value {
            Value::Grid(cells) if cells.size == T::SIZE => Self {
                cells: cells.clone(),
                marker: PhantomData,
            },
            _ => Self::default(),
        }
    }

    fn write(&self, _owner: ObjectId, _field: u16, _out: &mut Vec<(ObjectId, Object)>) -> Value {
        Value::Grid(self.cells.clone())
    }
}

impl<M, T: Cell> FieldRef<M, Grid<T>> {
    pub fn paint(self, object: ObjectId, cells: impl IntoIterator<Item = (i32, i32, T)>) -> Change {
        Change::Paint {
            object,
            field: self.index(),
            cells: cells
                .into_iter()
                .map(|(x, y, value)| Paint {
                    x,
                    y,
                    expected: None,
                    value: value.to_bytes(),
                })
                .collect(),
        }
    }

    pub fn reshape(self, object: ObjectId, bounds: Bounds) -> Change {
        Change::Reshape {
            object,
            field: self.index(),
            expected: None,
            bounds,
            cells: Vec::new(),
        }
    }
}
