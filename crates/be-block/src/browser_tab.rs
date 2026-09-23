use be_commit::MergeResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockContent, ContentError, LiveEdit, Merge};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryItem {
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BrowserTabContent {
    history: Vec<HistoryItem>,
    index: usize,
}

impl Default for BrowserTabContent {
    fn default() -> Self {
        Self::at("about:blank")
    }
}

impl BrowserTabContent {
    pub fn at(url: impl Into<String>) -> Self {
        Self {
            history: vec![HistoryItem {
                url: url.into(),
                title: String::new(),
            }],
            index: 0,
        }
    }

    pub fn history(&self) -> &[HistoryItem] {
        &self.history
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn current(&self) -> &HistoryItem {
        &self.history[self.index]
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BrowserTabOp {
    Push(HistoryItem),
    Replace(HistoryItem),
    History(usize),
}

impl BlockContent for BrowserTabContent {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7765_622d_6272_6f77_7365_722d_7461_6202);

    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let tab: Self =
            postcard::from_bytes(bytes).map_err(|_| ContentError::Malformed("browser tab"))?;
        if tab.index >= tab.history.len() {
            return Err(ContentError::Malformed(
                "a browser tab points past its history",
            ));
        }
        Ok(tab)
    }

    fn name(&self) -> Option<String> {
        let title = self.current().title.trim();
        (!title.is_empty()).then(|| title.to_owned())
    }
}

impl LiveEdit for BrowserTabContent {
    type Op = BrowserTabOp;

    fn apply(&mut self, operation: &Self::Op) {
        match operation {
            BrowserTabOp::Push(item) => {
                self.history.truncate(self.index.saturating_add(1));
                self.history.push(item.clone());
                self.index = self.history.len() - 1;
            }
            BrowserTabOp::Replace(item) => {
                if let Some(current) = self.history.get_mut(self.index) {
                    current.clone_from(item);
                }
            }
            BrowserTabOp::History(index) => {
                if *index < self.history.len() {
                    self.index = *index;
                }
            }
        }
    }
}

impl Merge for BrowserTabContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        if ours == base {
            MergeResult::Clean(theirs.clone())
        } else if theirs == base || theirs == ours {
            MergeResult::Clean(ours.clone())
        } else {
            MergeResult::Conflicted {
                value: ours.clone(),
                conflicts: 1,
            }
        }
    }
}
