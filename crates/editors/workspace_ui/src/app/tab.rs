use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TabItem {
    pub(crate) id: Uuid,
    pub(crate) block_type: Uuid,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct BlockTab {
    pub(crate) history: Vec<TabItem>,
    pub(crate) index: usize,
}

impl BlockTab {
    pub(crate) fn new(item: TabItem) -> Self {
        Self {
            history: vec![item],
            index: 0,
        }
    }

    pub(crate) fn current(&self) -> TabItem {
        self.history[self.index]
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }

    pub(crate) fn navigate(&mut self, item: TabItem) {
        if self.current().id == item.id {
            return;
        }
        self.history.truncate(self.index + 1);
        self.history.push(item);
        self.index += 1;
    }

    pub(crate) fn blocks(&self) -> Vec<Uuid> {
        self.history.iter().map(|item| item.id).collect()
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Navigation {
    Back,
    Forward,
    Open(TabItem),
}
