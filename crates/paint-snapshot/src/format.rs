use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use paint_format::{Frame, Painting, TextureKey};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"BE3PAINT";
const VERSION: u32 = 5;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub frames: Vec<Frame>,
    pub textures: BTreeMap<TextureKey, Texture>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Texture {
    pub size: [u32; 2],
    pub alpha: bool,
    #[serde(with = "serde_bytes")]
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

    pub fn of_painting(painting: Painting) -> Result<Self, String> {
        let mut keys = BTreeMap::new();
        let mut textures = BTreeMap::new();
        for (key, pixels) in painting.textures {
            let fingerprint = crate::fingerprint(pixels.size, &pixels.rgba);
            keys.insert(key, fingerprint);
            if let std::collections::btree_map::Entry::Vacant(entry) = textures.entry(fingerprint) {
                entry.insert(Texture::encode(pixels.size, &pixels.rgba)?);
            }
        }
        let rekey = |key: TextureKey| keys.get(&key).copied().unwrap_or(key);
        let mut frame = painting.frame;
        for primitive in &mut frame.primitives {
            match &mut primitive.content {
                paint_format::Content::Mesh(triangles) => {
                    for triangle in triangles {
                        triangle.texture = rekey(triangle.texture);
                    }
                }
                paint_format::Content::Glyph(glyph) => glyph.texture = rekey(glyph.texture),
                _ => {}
            }
        }
        Ok(Self::of(frame, textures))
    }

    pub fn append(&mut self, other: Self) {
        self.frames.extend(other.frames);
        self.textures.extend(other.textures);
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        Self::compress(&self.body()?)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        paint_format::decode(&Self::body_of(bytes)?)
            .map_err(|error| format!("could not deserialize snapshot: {error}"))
    }

    pub(crate) fn body(&self) -> Result<Vec<u8>, String> {
        paint_format::encode(self).map_err(|error| format!("could not serialize snapshot: {error}"))
    }

    pub(crate) fn compress(body: &[u8]) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::with_capacity(body.len() / 8);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        let mut encoder = DeflateEncoder::new(bytes, Compression::best());
        encoder
            .write_all(body)
            .map_err(|error| format!("could not compress snapshot: {error}"))?;
        encoder
            .finish()
            .map_err(|error| format!("could not compress snapshot: {error}"))
    }

    pub(crate) fn body_of(bytes: &[u8]) -> Result<Vec<u8>, String> {
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
        Ok(body)
    }
}
