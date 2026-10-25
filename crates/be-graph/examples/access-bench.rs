use std::time::Instant;

use be_graph::{Access, BlockGraph, BlockNode, BlockParent};
use uuid::Uuid;

const BLOCKS: u128 = 100_000;
const ACCOUNTS: u128 = 1_000;
const FOLDERS: u128 = 2_000;
const SAMPLES: usize = 2_000;

struct Random(u64);

impl Random {
    fn below(&mut self, bound: u128) -> u128 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        u128::from(self.0) % bound
    }
}

fn account(index: u128) -> Uuid {
    Uuid::from_u128((1 << 64) + index)
}

fn organization() -> BlockGraph {
    let mut random = Random(0x5eed);
    let mut graph = BlockGraph::new();
    let content = Uuid::from_u128(0xb10c);
    graph
        .insert(
            Uuid::from_u128(1),
            BlockNode::new(content, account(0), BlockParent::Root),
        )
        .unwrap();
    for block in 2..=BLOCKS {
        let parent = match block <= FOLDERS {
            true => 1 + random.below(block - 1),
            false => 1 + random.below(FOLDERS),
        };
        let node = BlockNode::new(
            content,
            account(random.below(ACCOUNTS)),
            BlockParent::Block(Uuid::from_u128(parent)),
        );
        graph.insert(Uuid::from_u128(block), node).unwrap();
    }
    for folder in 2..=FOLDERS {
        for _ in 0..3 {
            let level = [Access::KnowExists, Access::View, Access::Edit][random.below(3) as usize];
            graph
                .grant(
                    Uuid::from_u128(folder),
                    account(random.below(ACCOUNTS)),
                    level,
                )
                .unwrap();
        }
    }
    for _ in 0..BLOCKS / 10 {
        let from = Uuid::from_u128(1 + random.below(BLOCKS));
        let to = Uuid::from_u128(1 + random.below(BLOCKS));
        graph.set_references(from, &[to]).unwrap();
    }
    graph
}

fn time(metric: &str, label: &str, runs: usize, mut work: impl FnMut(usize) -> usize) {
    let start = Instant::now();
    let mut found = 0;
    for run in 0..runs {
        found += work(run);
    }
    let each = start.elapsed() / u32::try_from(runs).unwrap();
    bench::record(metric, each.as_secs_f64() * 1e6, "µs");
    println!(
        "{label:<44} {:>10.1} µs  ({found} found)",
        each.as_secs_f64() * 1e6
    );
}

fn main() {
    let start = Instant::now();
    let graph = organization();
    bench::time("build", start.elapsed());
    println!(
        "built {} blocks for {ACCOUNTS} accounts in {:?}",
        graph.len(),
        start.elapsed()
    );
    let mut random = Random(0xbe3);
    let pairs: Vec<(Uuid, Uuid)> = (0..SAMPLES)
        .map(|_| {
            (
                Uuid::from_u128(1 + random.below(BLOCKS)),
                account(random.below(ACCOUNTS)),
            )
        })
        .collect();
    time(
        "inherited",
        "inherited access of one block",
        SAMPLES,
        |run| {
            let (block, who) = pairs[run];
            usize::from(graph.inherited(block, who).can_view())
        },
    );
    time(
        "effective-leaf",
        "effective access of one leaf",
        SAMPLES,
        |run| {
            let (block, who) = pairs[run];
            usize::from(graph.effective_access(block, who).can_know_exists())
        },
    );
    time("visible", "what one account sees", 100, |run| {
        graph.visible(pairs[run].1).len()
    });
    time("audience-leaf", "audience of one leaf", SAMPLES, |run| {
        graph.audience(pairs[run].0).len()
    });
    time("audience-folder", "audience of one folder", 200, |run| {
        let folder = Uuid::from_u128(2 + (run as u128) % (FOLDERS - 1));
        graph.audience(folder).len()
    });
    time("root-children", "children of the root", 100, |_| {
        graph.children(BlockParent::Block(Uuid::from_u128(1))).len()
    });
}
