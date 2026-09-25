use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError};

use block_editor_beui::Waker;
use block_editor_beui::beui::{Image, Vec2};
use paint_snapshot::{Content, Snapshot};

mod difference;
#[cfg(test)]
mod inline;
#[cfg_attr(test, allow(dead_code))]
mod worker;

pub use difference::difference;

#[cfg(test)]
use inline::start as start_raster;
#[cfg(not(test))]
use worker::start as start_raster;

const BUDGET: usize = 48 * 1024 * 1024;

pub struct Painted {
    pub image: Image,
    pub description: String,
}

struct Held {
    image: Image,
    description: String,
}

impl Held {
    fn new(painted: Painted) -> Self {
        Self {
            image: painted.image,
            description: painted.description,
        }
    }

    fn shown(&self) -> Rendered {
        Rendered {
            size: self.image.size(),
            image: self.image.clone(),
            description: self.description.clone(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Rendered {
    pub image: Image,
    pub size: Vec2,
    pub description: String,
}

pub struct Change {
    pub description: String,
    pub frame: Option<usize>,
}

pub enum Message {
    Frames(usize),
    Painted(usize, Result<Painted, String>),
    Broken(String),
}

#[derive(Default)]
struct Reel {
    frames: Vec<Option<Result<Held, String>>>,
    bytes: usize,
}

impl Reel {
    fn done(&self) -> usize {
        self.frames.iter().filter(|frame| frame.is_some()).count()
    }
}

struct Job {
    hash: String,
    messages: Receiver<Message>,
}

#[derive(Default)]
pub struct Paintings {
    cached: Vec<(String, Reel)>,
    active: Option<Job>,
    queue: VecDeque<(String, Vec<u8>)>,
    kept: Vec<String>,
    rastered: usize,
}

impl Paintings {
    pub fn want(&mut self, hash: &str, data: Vec<u8>) {
        self.touch(hash);
        if self.holds(hash) {
            return;
        }
        self.queue.push_back((hash.to_owned(), data));
    }

    pub fn keep(&mut self, hashes: Vec<String>) {
        self.kept = hashes;
        self.evict();
    }

    pub fn holds(&self, hash: &str) -> bool {
        self.cached.iter().any(|(seen, _)| seen == hash)
            || self.queue.iter().any(|(seen, _)| seen == hash)
            || self.active.as_ref().is_some_and(|job| job.hash == hash)
    }

    pub fn settle(&mut self, waker: &Waker) -> bool {
        let finished = self.receive();
        if self.active.is_none()
            && !finished
            && let Some((hash, data)) = self.queue.pop_front()
        {
            self.active = Some(Job {
                messages: start_raster(data, waker.clone()),
                hash,
            });
        }
        let working = self.active.is_some() || !self.queue.is_empty();
        if working {
            waker.wake();
        }
        finished || working
    }

    pub fn count(&self, hash: &str) -> Option<usize> {
        let reel = self.reel(hash)?;
        (!reel.frames.is_empty()).then_some(reel.frames.len())
    }

    pub fn loading(&self, hash: &str) -> Option<(usize, usize)> {
        let reel = self.reel(hash)?;
        let done = reel.done();
        (done < reel.frames.len()).then_some((done, reel.frames.len()))
    }

    pub fn rendered(&mut self, hash: &str, frame: usize) -> Option<Result<Rendered, String>> {
        self.touch(hash);
        let held = self.held(hash)?.frames.get(frame)?.as_ref()?;
        Some(match held {
            Ok(held) => Ok(held.shown()),
            Err(error) => Err(error.clone()),
        })
    }

    pub fn computed(
        &mut self,
        hash: &str,
        frame: usize,
        count: usize,
        paint: impl FnOnce() -> Result<Painted, String>,
    ) -> Result<Rendered, String> {
        if let Some(held) = self.rendered(hash, frame) {
            return held;
        }
        let painted = paint();
        let reel = self.reel_mut(hash);
        reel.frames.resize_with(count.max(frame + 1), || None);
        reel.frames[frame] = Some(painted.map(Held::new));
        reel.bytes = held_bytes(reel);
        let rendered = self
            .rendered(hash, frame)
            .expect("the frame was just painted");
        self.evict();
        rendered
    }

    pub fn rastered(&self) -> usize {
        self.rastered
    }

    fn reel(&self, hash: &str) -> Option<&Reel> {
        self.cached
            .iter()
            .find(|(seen, _)| seen == hash)
            .map(|(_, reel)| reel)
    }

    fn held(&mut self, hash: &str) -> Option<&mut Reel> {
        self.cached
            .iter_mut()
            .find(|(seen, _)| seen == hash)
            .map(|(_, reel)| reel)
    }

    fn reel_mut(&mut self, hash: &str) -> &mut Reel {
        if !self.cached.iter().any(|(seen, _)| seen == hash) {
            self.cached.push((hash.to_owned(), Reel::default()));
        }
        self.held(hash).expect("the reel was just inserted")
    }

    fn touch(&mut self, hash: &str) {
        if let Some(index) = self.cached.iter().position(|(seen, _)| seen == hash) {
            let entry = self.cached.remove(index);
            self.cached.push(entry);
        }
    }

    fn receive(&mut self) -> bool {
        loop {
            let Some(job) = &self.active else {
                return false;
            };
            match job.messages.try_recv() {
                Ok(message) => {
                    let hash = job.hash.clone();
                    self.apply(&hash, message);
                }
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => {
                    self.active = None;
                    return true;
                }
            }
        }
    }

    fn apply(&mut self, hash: &str, message: Message) {
        match message {
            Message::Frames(count) => {
                let reel = self.reel_mut(hash);
                reel.frames.clear();
                reel.frames.resize_with(count, || None);
            }
            Message::Painted(index, painted) => {
                self.rastered += 1;
                let held = painted.map(Held::new);
                let reel = self.reel_mut(hash);
                if index >= reel.frames.len() {
                    reel.frames.resize_with(index + 1, || None);
                }
                reel.frames[index] = Some(held);
                reel.bytes = held_bytes(reel);
            }
            Message::Broken(error) => {
                let reel = self.reel_mut(hash);
                reel.frames = vec![Some(Err(error))];
                reel.bytes = 0;
            }
        }
        self.evict();
    }

    fn evict(&mut self) {
        let active = self.active.as_ref().map(|job| job.hash.clone());
        while self.bytes() > BUDGET {
            let kept = &self.kept;
            let Some(index) = self
                .cached
                .iter()
                .position(|(hash, _)| Some(hash) != active.as_ref() && !kept.contains(hash))
            else {
                break;
            };
            self.cached.remove(index);
        }
    }

    fn bytes(&self) -> usize {
        self.cached.iter().map(|(_, reel)| reel.bytes).sum()
    }
}

fn held_bytes(reel: &Reel) -> usize {
    reel.frames
        .iter()
        .flatten()
        .filter_map(|frame| frame.as_ref().ok())
        .map(|held| held.image.pixels().len())
        .sum()
}

pub fn paint_all(data: &[u8], send: &mut impl FnMut(Message)) {
    let snapshot = match Snapshot::decode(data) {
        Ok(snapshot) => snapshot,
        Err(error) => return send(Message::Broken(error)),
    };
    if snapshot.frames.is_empty() {
        return send(Message::Broken("the recording has no frames".to_owned()));
    }
    send(Message::Frames(snapshot.frames.len()));
    for frame in 0..snapshot.frames.len() {
        send(Message::Painted(frame, paint(&snapshot, frame)));
    }
}

pub fn describe(snapshot: &Snapshot, frame: usize) -> Result<String, String> {
    let frame = snapshot.frame(frame)?;
    let [width, height] = frame.size;
    let textures: std::collections::BTreeSet<_> = frame
        .primitives
        .iter()
        .flat_map(|primitive| match &primitive.content {
            Content::Mesh(triangles) => triangles.iter().map(|triangle| triangle.texture).collect(),
            Content::Glyph(glyph) => vec![glyph.texture],
            Content::Callback(_) | Content::RoundedRect(_) => Vec::new(),
        })
        .collect();
    Ok(format!(
        "{width}x{height}, {} draw calls, {} textures",
        frame.primitives.len(),
        textures.len()
    ))
}

pub fn change(approved: &[u8], current: &[u8]) -> Result<Change, String> {
    let (approved, current) = (Snapshot::decode(approved)?, Snapshot::decode(current)?);
    Ok(match paint_snapshot::difference(&approved, &current) {
        None => Change {
            description: "the painting is the same".to_owned(),
            frame: None,
        },
        Some(difference) => Change {
            description: difference.description,
            frame: difference.frame,
        },
    })
}

fn paint(snapshot: &Snapshot, frame: usize) -> Result<Painted, String> {
    let image = paint_snapshot::render(snapshot, frame)?;
    Ok(Painted {
        image: Image::from_rgba(image.width(), image.height(), image.into_raw()),
        description: describe(snapshot, frame)?,
    })
}
