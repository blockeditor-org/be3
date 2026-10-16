use std::collections::HashMap;

use be_block::{ARTIFACT, BlockMetadata, LOCAL_ID};
use be_protocol::BlockSummary;
use be_store::ObjectStore;
use uuid::Uuid;

use crate::{ClientError, Peer, Saved as Published};

pub const EXTENSION: &str = "block";

pub const MIME_TYPE: &str = "application/x-be3-block";

const MAGIC: &[u8] = b"be3 block file\n";
const VERSION: u32 = 1;
const TOP: u32 = u32::MAX;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub id: Uuid,
    pub block_type: Uuid,
    pub parent: Option<usize>,
    pub metadata: BlockMetadata,
    pub references: Vec<Uuid>,
    pub content: Option<Vec<u8>>,
}

pub fn encode(blocks: &[Saved]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend(VERSION.to_le_bytes());
    out.extend((blocks.len() as u32).to_le_bytes());
    for block in blocks {
        out.extend(block.id.as_bytes());
        out.extend(block.block_type.as_bytes());
        out.extend(
            block
                .parent
                .map_or(TOP, |parent| parent as u32)
                .to_le_bytes(),
        );
        let metadata = block.metadata.encode();
        out.extend((metadata.len() as u32).to_le_bytes());
        out.extend(metadata);
        out.extend((block.references.len() as u32).to_le_bytes());
        for reference in &block.references {
            out.extend(reference.as_bytes());
        }
        match &block.content {
            Some(content) => {
                out.push(1);
                out.extend((content.len() as u64).to_le_bytes());
                out.extend(content);
            }
            None => out.push(0),
        }
    }
    out
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Saved>, String> {
    let mut reader = Reader { bytes, at: 0 };
    if reader.take(MAGIC.len())? != MAGIC {
        return Err("this is not a block file".to_owned());
    }
    let version = reader.u32()?;
    if version != VERSION {
        return Err(format!(
            "this block file is version {version}, and this be reads version {VERSION}"
        ));
    }
    let count = reader.u32()? as usize;
    let mut blocks = Vec::new();
    for index in 0..count {
        let id = reader.uuid()?;
        let block_type = reader.uuid()?;
        let parent = match reader.u32()? {
            TOP => None,
            parent if (parent as usize) < index => Some(parent as usize),
            _ => return Err("a block in the file comes before its parent".to_owned()),
        };
        let length = reader.u32()? as usize;
        let metadata = BlockMetadata::decode(reader.take(length)?)
            .map_err(|error| format!("a block in the file has broken metadata: {error}"))?;
        let mut references = Vec::new();
        for _ in 0..reader.u32()? {
            references.push(reader.uuid()?);
        }
        let content = match reader.take(1)?[0] {
            0 => None,
            _ => {
                let length = usize::try_from(reader.u64()?)
                    .map_err(|_| "a block in the file is too large".to_owned())?;
                Some(reader.take(length)?.to_vec())
            }
        };
        blocks.push(Saved {
            id,
            block_type,
            parent,
            metadata,
            references,
            content,
        });
    }
    let mut seen = std::collections::HashSet::new();
    if blocks.iter().any(|block| !seen.insert(block.id)) {
        return Err("the block file holds one block twice".to_owned());
    }
    if reader.at != bytes.len() {
        return Err("the block file has more in it than its blocks".to_owned());
    }
    if blocks.is_empty() || blocks.iter().skip(1).any(|block| block.parent.is_none()) {
        return Err("a block file holds one block and what is inside it".to_owned());
    }
    Ok(blocks)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or("the block file ends early")?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn uuid(&mut self) -> Result<Uuid, String> {
        Ok(Uuid::from_slice(self.take(16)?).unwrap())
    }
}

pub fn swap_ids(bytes: &[u8], ids: &HashMap<Uuid, Uuid>) -> Vec<u8> {
    let mut forms: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    for (old, new) in ids {
        forms.insert(old.as_bytes().to_vec(), new.as_bytes().to_vec());
        forms.insert(
            old.hyphenated().to_string().into_bytes(),
            new.hyphenated().to_string().into_bytes(),
        );
        forms.insert(
            old.simple().to_string().into_bytes(),
            new.simple().to_string().into_bytes(),
        );
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    'scan: while at < bytes.len() {
        for length in [36, 32, 16] {
            if let Some(window) = bytes.get(at..at + length)
                && let Some(new) = forms.get(window)
            {
                out.extend(new);
                at += length;
                continue 'scan;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    out
}

pub fn file_name(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character => character,
        })
        .collect();
    match stem.is_empty() {
        true => format!("block.{EXTENSION}"),
        false => format!("{stem}.{EXTENSION}"),
    }
}

pub fn renumber(blocks: Vec<Saved>) -> Vec<Saved> {
    let ids: HashMap<Uuid, Uuid> = blocks
        .iter()
        .map(|block| (block.id, Uuid::now_v7()))
        .collect();
    blocks
        .into_iter()
        .map(|block| {
            let mut metadata = block.metadata;
            metadata.remove(&ARTIFACT);
            metadata.remove(&LOCAL_ID);
            Saved {
                id: ids[&block.id],
                block_type: block.block_type,
                parent: block.parent,
                metadata,
                references: block
                    .references
                    .iter()
                    .map(|reference| ids.get(reference).copied().unwrap_or(*reference))
                    .collect(),
                content: block.content.map(|bytes| swap_ids(&bytes, &ids)),
            }
        })
        .collect()
}

impl<S: ObjectStore> Peer<S> {
    pub async fn head_content(
        &self,
        summary: &BlockSummary,
    ) -> Result<Option<Vec<u8>>, ClientError> {
        let Some(head) = summary.head else {
            return Ok(None);
        };
        let commit = self.load_commit(head).await?;
        Ok(Some(self.read_manifest(&commit.manifest).await?))
    }

    pub async fn download_block(
        &self,
        block: Uuid,
        block_type: Uuid,
        parent: Option<usize>,
    ) -> Result<Saved, ClientError> {
        let summary = self.summary(block).await?;
        Ok(Saved {
            id: block,
            block_type,
            parent,
            metadata: self.metadata_of_summary(&summary).await?,
            content: self.head_content(&summary).await?,
            references: summary.references,
        })
    }

    pub async fn publish_content(&self, block: &Saved) -> Result<Option<Published>, ClientError> {
        let Some(content) = &block.content else {
            return Ok(None);
        };
        let manifest = self
            .write_content(block.id, block.block_type, content)
            .await?;
        self.publish_manifest(
            block.id,
            manifest,
            block.references.clone(),
            None,
            be_commit::now_milliseconds(),
            false,
            Vec::new(),
            &block.metadata,
        )
        .await
        .map(Some)
    }
}
