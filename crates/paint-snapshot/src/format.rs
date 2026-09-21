use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"BE3PAINT";
const VERSION: u32 = 4;

pub type TextureKey = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub frames: Vec<Frame>,
    pub textures: BTreeMap<TextureKey, Texture>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub size: [u32; 2],
    pub pixels_per_point: f32,
    pub background: [u8; 4],
    pub primitives: Vec<Primitive>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Primitive {
    pub clip: [f32; 4],
    pub content: Content,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Mesh(Vec<Triangle>),
    Callback([f32; 4]),
    RoundedRect(RoundedRect),
    Glyph(Glyph),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundedRect {
    pub rect: [f32; 4],
    pub corner_radius: f32,
    pub stroke_width: f32,
    pub color: [u8; 4],
    pub turn: Turn,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glyph {
    pub rect: [f32; 4],
    pub texture: TextureKey,
    pub color: [u8; 4],
    pub turn: Turn,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub pivot: [f32; 2],
    pub angle: f32,
}

impl Turn {
    pub const NONE: Self = Self {
        pivot: [0.0, 0.0],
        angle: 0.0,
    };

    pub fn turns(self) -> bool {
        self.angle != 0.0
    }

    pub fn scaled(self, scale: f32) -> Self {
        Self {
            pivot: [self.pivot[0] * scale, self.pivot[1] * scale],
            angle: self.angle,
        }
    }

    pub fn undo(self, point: [f32; 2]) -> [f32; 2] {
        if !self.turns() {
            return point;
        }
        let (sin, cos) = (-self.angle).sin_cos();
        let (x, y) = (point[0] - self.pivot[0], point[1] - self.pivot[1]);
        [
            self.pivot[0] + x * cos - y * sin,
            self.pivot[1] + x * sin + y * cos,
        ]
    }

    pub fn swept(self, rect: [f32; 4]) -> [f32; 4] {
        if !self.turns() {
            return rect;
        }
        let (sin, cos) = self.angle.sin_cos();
        let turned = |x: f32, y: f32| {
            let (x, y) = (x - self.pivot[0], y - self.pivot[1]);
            [
                self.pivot[0] + x * cos - y * sin,
                self.pivot[1] + x * sin + y * cos,
            ]
        };
        [
            turned(rect[0], rect[1]),
            turned(rect[2], rect[1]),
            turned(rect[2], rect[3]),
            turned(rect[0], rect[3]),
        ]
        .into_iter()
        .fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |bounds, [x, y]| {
                [
                    bounds[0].min(x),
                    bounds[1].min(y),
                    bounds[2].max(x),
                    bounds[3].max(y),
                ]
            },
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Triangle {
    pub texture: TextureKey,
    pub corners: [Vertex; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Texture {
    pub size: [u32; 2],
    pub alpha: bool,
    pub png: Vec<u8>,
}

impl Texture {
    pub fn pixels(&self) -> Result<Vec<[u8; 4]>, String> {
        let image = image::load_from_memory_with_format(&self.png, image::ImageFormat::Png)
            .map_err(|error| format!("texture is not a readable png: {error}"))?;
        if self.alpha {
            return Ok(image
                .to_luma8()
                .pixels()
                .map(|pixel| [pixel.0[0]; 4])
                .collect());
        }
        Ok(image.to_rgba8().pixels().map(|pixel| pixel.0).collect())
    }

    pub fn encode(size: [u32; 2], pixels: &[[u8; 4]]) -> Result<Self, String> {
        let alpha = pixels
            .iter()
            .all(|pixel| pixel.iter().all(|channel| *channel == pixel[3]));
        let mut png = Vec::new();
        let target = &mut std::io::Cursor::new(&mut png);
        if alpha {
            let flat = pixels.iter().map(|pixel| pixel[3]).collect();
            image::GrayImage::from_vec(size[0], size[1], flat)
                .ok_or("texture pixels do not match its size")?
                .write_to(target, image::ImageFormat::Png)
        } else {
            let flat = pixels.iter().flatten().copied().collect();
            image::RgbaImage::from_vec(size[0], size[1], flat)
                .ok_or("texture pixels do not match its size")?
                .write_to(target, image::ImageFormat::Png)
        }
        .map_err(|error| format!("could not encode texture: {error}"))?;
        Ok(Self { size, alpha, png })
    }
}

impl Snapshot {
    pub fn of(frame: Frame, textures: BTreeMap<TextureKey, Texture>) -> Self {
        Self {
            frames: vec![frame],
            textures,
        }
    }

    pub fn frame(&self, index: usize) -> Result<&Frame, String> {
        self.frames.get(index).ok_or_else(|| {
            format!(
                "the recording has {} frames, there is no frame {}",
                self.frames.len(),
                index + 1
            )
        })
    }

    pub fn append(&mut self, other: Self) {
        self.frames.extend(other.frames);
        self.textures.extend(other.textures);
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let body = bincode::serialize(self)
            .map_err(|error| format!("could not serialize snapshot: {error}"))?;
        let mut bytes = Vec::with_capacity(body.len() / 2);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        let mut encoder = DeflateEncoder::new(bytes, Compression::best());
        encoder
            .write_all(&body)
            .map_err(|error| format!("could not compress snapshot: {error}"))?;
        encoder
            .finish()
            .map_err(|error| format!("could not compress snapshot: {error}"))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let header = bytes.len().min(MAGIC.len() + 4);
        if header < MAGIC.len() + 4 || &bytes[..MAGIC.len()] != MAGIC {
            return Err("not a paint snapshot".into());
        }
        let version = u32::from_le_bytes(bytes[MAGIC.len()..header].try_into().unwrap());
        if version != VERSION {
            return Err(format!(
                "snapshot is version {version}, this tool reads version {VERSION}"
            ));
        }
        let mut body = Vec::new();
        DeflateDecoder::new(&bytes[header..])
            .read_to_end(&mut body)
            .map_err(|error| format!("could not decompress snapshot: {error}"))?;
        bincode::deserialize(&body)
            .map_err(|error| format!("could not deserialize snapshot: {error}"))
    }
}
