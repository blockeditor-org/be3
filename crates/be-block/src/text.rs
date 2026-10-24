use std::ops::Range;

use be_model::{Document, Edit, Model, ObjectId, Sequence, Text};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

const MAX_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextLanguage {
    #[default]
    Markdown,
    PlainText,
    Rust,
    Zig,
    Toml,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextIndentation {
    Tabs,
    Spaces {
        #[serde(default = "two")]
        width: u8,
    },
}

fn two() -> u8 {
    2
}

impl Default for TextIndentation {
    fn default() -> Self {
        Self::Spaces { width: 2 }
    }
}

impl TextIndentation {
    fn normalized(self) -> Self {
        match self {
            Self::Tabs => Self::Tabs,
            Self::Spaces { width } => Self::Spaces {
                width: width.max(1),
            },
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "text")]
pub struct TextBlock {
    pub language: TextLanguage,
    pub indentation: TextIndentation,
    pub body: Text,
}

pub type TextContent = Document<TextBlock>;

impl TextBlock {
    pub fn new(text: impl Into<Vec<u8>>) -> Self {
        Self {
            body: Text::new(text),
            ..Self::default()
        }
    }

    pub fn with_language(mut self, language: TextLanguage) -> Self {
        self.language = language;
        self
    }

    pub fn content(&self) -> TextContent {
        Document::new(self)
    }

    pub fn of(text: &str) -> TextContent {
        Self::new(text).content()
    }

    pub fn body(content: &TextContent) -> Option<&Sequence<u8>> {
        content.text(ObjectId::ROOT, Self::BODY)
    }

    pub fn text(content: &TextContent) -> String {
        Self::body(content).map_or_else(String::new, |body| {
            String::from_utf8_lossy(&body.items()).into_owned()
        })
    }

    pub fn insert(content: &TextContent, client: u64, at: usize, bytes: &[u8]) -> Option<Edit> {
        let op = Self::body(content)?.insert(client, at, bytes.to_vec())?;
        Some(Self::BODY.edit(ObjectId::ROOT, op).into())
    }

    pub fn delete(content: &TextContent, range: Range<usize>) -> Option<Edit> {
        let op = Self::body(content)?.delete(range)?;
        Some(Self::BODY.edit(ObjectId::ROOT, op).into())
    }

    pub fn replace(
        content: &TextContent,
        client: u64,
        range: Range<usize>,
        bytes: &[u8],
    ) -> Option<Edit> {
        let op = Self::body(content)?.replace(client, range, bytes.to_vec())?;
        Some(Self::BODY.edit(ObjectId::ROOT, op).into())
    }

    pub fn rewrite(content: &TextContent, client: u64, bytes: &[u8]) -> Option<Edit> {
        let now = Self::body(content)?.items();
        if now == bytes {
            return None;
        }
        let boundary = |text: &[u8], at: usize| text.get(at).is_none_or(|byte| byte & 0xc0 != 0x80);
        let mut start = now.iter().zip(bytes).take_while(|(a, b)| a == b).count();
        while start > 0 && !(boundary(&now, start) && boundary(bytes, start)) {
            start -= 1;
        }
        let mut tail = now[start..]
            .iter()
            .rev()
            .zip(bytes[start..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        while tail > 0 && !(boundary(&now, now.len() - tail) && boundary(bytes, bytes.len() - tail))
        {
            tail -= 1;
        }
        Self::replace(
            content,
            client,
            start..now.len() - tail,
            &bytes[start..bytes.len() - tail],
        )
    }

    pub fn set_language(language: TextLanguage) -> Edit {
        Self::LANGUAGE.set(ObjectId::ROOT, &language).into()
    }

    pub fn set_indentation(indentation: TextIndentation) -> Edit {
        Self::INDENTATION
            .set(ObjectId::ROOT, &indentation.normalized())
            .into()
    }

    pub fn linked_blocks(&self, organization: Option<Uuid>) -> Vec<Uuid> {
        let mut seen = std::collections::HashSet::new();
        crate::block_url::parse_block_urls(&self.body)
            .into_iter()
            .filter(|url| {
                organization.is_none_or(|organization| url.organization_id == organization)
            })
            .map(|url| url.block)
            .filter(|id| seen.insert(*id))
            .collect()
    }
}

impl Root for TextBlock {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_014b);

    const FROZEN: bool = true;

    fn name(&self) -> Option<String> {
        let line = self.body.split(|byte| *byte == b'\n').next()?;
        let mut name = String::new();
        for character in String::from_utf8_lossy(line).trim().chars() {
            if name.len() + character.len_utf8() > MAX_NAME_BYTES {
                break;
            }
            name.push(character);
        }
        (!name.is_empty()).then_some(name)
    }

    fn references(&self) -> Vec<Uuid> {
        self.linked_blocks(None)
    }

    fn references_in(&self, organization: Uuid) -> Vec<Uuid> {
        self.linked_blocks(Some(organization))
    }
}
