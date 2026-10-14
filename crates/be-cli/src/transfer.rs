use std::collections::{HashMap, VecDeque};

use be_block::database::{Database, DatabaseContent};
use be_block::{BlockContent, BlockMetadata};
use be_graph::BlockParent;
use be_store::ObjectStore;
use uuid::Uuid;

use crate::blocks::Entry;
use crate::session::explain;
use crate::tracker::Tracker;

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
        return Err("this is not a file `be download` wrote".to_owned());
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

pub struct Uploaded {
    pub top: Uuid,
    pub blocks: usize,
    pub renamed_prefix: Option<(String, String)>,
}

impl<S: ObjectStore> Tracker<S> {
    pub async fn download(&self, entry: &Entry) -> Result<(Entry, Vec<Saved>), String> {
        let top = self.outermost(entry).await?;
        let index = self.index().await?;
        let mut order = vec![(top.clone(), None)];
        let mut queue = VecDeque::from([(top.id, 0)]);
        while let Some((parent, position)) = queue.pop_front() {
            for child in index.children(Some(parent)) {
                queue.push_back((child.id, order.len()));
                order.push((child.clone(), Some(position)));
            }
        }
        let peer = self.peer();
        let mut blocks = Vec::new();
        for (entry, parent) in order {
            let summary = peer.summary(entry.id).await.map_err(explain)?;
            let metadata = peer.metadata(&summary).map_err(explain)?;
            let content = match summary.head {
                Some(head) => {
                    let commit = peer.load_commit(head).await.map_err(explain)?;
                    peer.fetch(&commit.manifest.chunk_hashes())
                        .await
                        .map_err(explain)?;
                    Some(
                        peer.commits()
                            .vault()
                            .read(&commit.manifest)
                            .map_err(|error| error.to_string())?,
                    )
                }
                None => None,
            };
            blocks.push(Saved {
                id: entry.id,
                block_type: entry.block_type,
                parent,
                metadata,
                references: summary.references,
                content,
            });
        }
        Ok((top, blocks))
    }

    pub async fn upload(
        &self,
        blocks: Vec<Saved>,
        parent: BlockParent,
        prefix: Option<&str>,
    ) -> Result<Uploaded, String> {
        let ids: HashMap<Uuid, Uuid> = blocks
            .iter()
            .map(|block| (block.id, Uuid::new_v4()))
            .collect();
        let mut prepared = Vec::new();
        let mut taken = Vec::new();
        for block in blocks {
            let mut metadata = block.metadata.clone();
            metadata.remove(&be_block::ARTIFACT);
            let content = block.content.as_deref().map(|bytes| swap_ids(bytes, &ids));
            if block.block_type == DatabaseContent::CONTENT_TYPE
                && let Some(bytes) = &content
            {
                let database = DatabaseContent::decode(bytes)
                    .map_err(|error| format!("a database in the file is unreadable: {error}"))?;
                let held = database.root().prefix.trim().to_owned();
                if !held.is_empty() && !matches!(self.database_with_prefix(&held).await, Ok(None)) {
                    taken.push((prepared.len(), held, database));
                }
            }
            prepared.push((block, metadata, content));
        }
        let renamed_prefix = match (taken.as_mut_slice(), prefix) {
            ([], _) => None,
            ([(position, held, database)], Some(wanted)) => {
                let wanted = wanted.trim().to_ascii_uppercase();
                if !wanted.is_empty() && self.database_with_prefix(&wanted).await?.is_some() {
                    return Err(format!("another database already uses the prefix {wanted}"));
                }
                database.apply(&Database::set_prefix(wanted.clone()));
                let (_, metadata, content) = &mut prepared[*position];
                metadata.insert(&be_block::ROW_PREFIX, &wanted);
                *content = Some(database.encode());
                Some((held.clone(), wanted))
            }
            ([(_, held, _)], None) => {
                return Err(format!(
                    "another database already uses the row id prefix {held}; give the copy its own with --prefix"
                ));
            }
            (several, _) => {
                return Err(format!(
                    "other databases already use the row id prefixes {}",
                    several
                        .iter()
                        .map(|(_, held, _)| held.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        };
        let peer = self.peer();
        let made: Vec<Uuid> = prepared.iter().map(|(block, ..)| ids[&block.id]).collect();
        for (block, metadata, content) in &prepared {
            let id = ids[&block.id];
            let placed = match block.parent {
                Some(position) => BlockParent::Block(made[position]),
                None => parent,
            };
            peer.create_block(id, block.block_type, placed, metadata)
                .await
                .map_err(explain)?;
            let Some(content) = content else {
                continue;
            };
            let manifest = peer
                .commits()
                .vault()
                .write(block.block_type, content)
                .map_err(|error| error.to_string())?;
            let references = block
                .references
                .iter()
                .map(|reference| ids.get(reference).copied().unwrap_or(*reference))
                .collect();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_millis() as i64);
            peer.publish_manifest(
                id,
                manifest,
                references,
                None,
                now,
                false,
                Vec::new(),
                metadata,
            )
            .await
            .map_err(explain)?;
        }
        self.invalidate().await;
        Ok(Uploaded {
            top: made[0],
            blocks: made.len(),
            renamed_prefix,
        })
    }
}
