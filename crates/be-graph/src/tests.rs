use super::*;

mod a_cycle_is_refused_when_reparenting;
mod a_detached_subtree_is_pending_deletion;
mod a_kind_an_editor_changes_hangs_from_its_owner;
mod a_kind_open_inside_is_read_by_anyone_who_views_a_block_inside;
mod a_lower_grant_never_reduces_inherited_access;
mod a_parent_holds_a_child_only_while_it_references_it;
mod a_parent_knows_its_children_as_the_graph_changes;
mod a_settings_block_needs_manage_on_its_parent_to_change;
mod access_is_inherited_through_parents;
mod access_names_the_block_it_comes_from;
mod access_to_one_block_agrees_with_mapping_the_whole_graph;
mod an_account_sees_what_lies_below_its_entry_points;
mod an_author_edits_only_what_lies_where_it_can_see;
mod an_author_keeps_no_more_of_a_settings_kind_than_its_settings;
mod object_references_are_counted_not_traced;
mod references_do_not_keep_a_block_alive;
mod the_audience_of_a_block_holds_everyone_who_may_know_it;

const TYPE: Uuid = Uuid::from_u128(0xb10c);
const AUTHOR: Uuid = Uuid::from_u128(0xa170);

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn graph(parents: &[(u128, BlockParent)]) -> BlockGraph {
    let mut graph = BlockGraph::new();
    for (block, parent) in parents {
        graph
            .insert(id(*block), BlockNode::new(TYPE, AUTHOR, *parent))
            .unwrap();
        if let BlockParent::Block(parent) = parent {
            let mut references = graph.references(*parent);
            references.push(id(*block));
            graph.set_references(*parent, &references).unwrap();
        }
    }
    graph
}

const ACCOUNTS: [Uuid; 4] = [
    Uuid::from_u128(0xacc0),
    Uuid::from_u128(0xacc1),
    Uuid::from_u128(0xacc2),
    Uuid::from_u128(0xacc3),
];

struct Random(u64);

impl Random {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap()).unwrap()
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn account(&mut self) -> Uuid {
        ACCOUNTS[self.below(ACCOUNTS.len())]
    }

    fn level(&mut self) -> Access {
        [
            Access::KnowExists,
            Access::View,
            Access::Edit,
            Access::Manage,
        ][self.below(4)]
    }

    fn block(&mut self, graph: &BlockGraph) -> Option<Uuid> {
        let ids = graph.ids();
        (!ids.is_empty()).then(|| ids[self.below(ids.len())])
    }
}

fn random_graph(seed: u64, size: u128) -> (BlockGraph, Random) {
    let mut random = Random(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut graph = BlockGraph::new();
    for block in 1..=size {
        let parent = match random.below(10) {
            0 => BlockParent::Detached,
            1 | 2 => BlockParent::Root,
            _ => random
                .block(&graph)
                .map_or(BlockParent::Root, BlockParent::Block),
        };
        let content_type = match random.below(20) {
            0..=2 => SETTINGS_TYPE,
            3 => TAGS_TYPE,
            _ => TYPE,
        };
        let mut node = BlockNode::new(content_type, random.account(), parent);
        if random.chance(25) {
            node.grants.insert(random.account(), random.level());
        }
        if random.chance(20) {
            node.grants.insert(ANYONE_INSIDE, random.level());
        }
        graph.insert(id(block), node).unwrap();
    }
    for block in graph.ids() {
        let mut references = Vec::new();
        while random.chance(30) {
            references.extend(random.block(&graph));
        }
        graph.set_references(block, &references).unwrap();
    }
    (graph, random)
}

fn mutate(graph: &mut BlockGraph, random: &mut Random, next: &mut u128) {
    let Some(block) = random.block(graph) else {
        return;
    };
    match random.below(6) {
        0 => {
            let parent = match random.below(4) {
                0 => BlockParent::Detached,
                1 => BlockParent::Root,
                _ => BlockParent::Block(random.block(graph).unwrap()),
            };
            let _ = graph.set_parent(block, parent);
        }
        1 => {
            let level = match random.chance(30) {
                true => Access::None,
                false => random.level(),
            };
            let account = match random.chance(20) {
                true => ANYONE_INSIDE,
                false => random.account(),
            };
            graph.grant(block, account, level).unwrap();
        }
        2 => {
            let references: Vec<Uuid> = (0..random.below(3))
                .filter_map(|_| random.block(graph))
                .collect();
            graph.set_references(block, &references).unwrap();
        }
        3 => {
            graph.remove(block);
            assert!(
                graph
                    .ids()
                    .iter()
                    .all(|other| !graph.references(*other).contains(&block)),
                "a removed block is still referenced"
            );
        }
        _ => {
            *next += 1;
            let parent = match random.chance(30) {
                true => BlockParent::Root,
                false => BlockParent::Block(block),
            };
            graph
                .insert(id(*next), BlockNode::new(TYPE, random.account(), parent))
                .unwrap();
        }
    }
}

fn varied_graphs(mut check: impl FnMut(&BlockGraph)) {
    for seed in 0..40 {
        let (mut graph, mut random) = random_graph(seed, 30);
        let mut next = 1000;
        check(&graph);
        for _ in 0..30 {
            mutate(&mut graph, &mut random, &mut next);
            check(&graph);
        }
    }
}

fn whole_graph_access_map(graph: &BlockGraph, account: Uuid) -> BTreeMap<Uuid, Access> {
    fn level(
        graph: &BlockGraph,
        account: Uuid,
        id: Uuid,
        memo: &mut BTreeMap<Uuid, Access>,
    ) -> Access {
        if let Some(level) = memo.get(&id) {
            return *level;
        }
        let node = graph.get(id).unwrap();
        let (parent, parent_viewable) = match node.parent {
            BlockParent::Block(parent) => {
                let above = level(graph, account, parent, memo);
                (above, above.can_view())
            }
            BlockParent::Root | BlockParent::Detached => (Access::None, true),
        };
        let mut best = node.grants.get(&account).copied().unwrap_or_default();
        if node.parent.block().is_some_and(|parent| in_settings(graph, parent)) {
            best = match parent.can_view() {
                true => parent,
                false => best.min(Access::KnowExists),
            };
        } else if parent.can_view() {
            best = best.max(parent);
        }
        if node.author == account
            && parent_viewable
            && !node.parent.block().is_some_and(|parent| in_settings(graph, parent))
        {
            best = Access::Manage;
        }
        if node.content_type == TAGS_TYPE && matches!(node.parent, BlockParent::Block(_)) {
            best = match parent {
                above if above.can_view() => best.min(above.max(Access::View)),
                _ => best.min(Access::KnowExists),
            };
        }
        if node.content_type == SETTINGS_TYPE && matches!(node.parent, BlockParent::Block(_)) {
            best = match parent {
                Access::Manage => Access::Manage,
                above if above.can_view() => best.min(Access::View),
                _ => best.min(Access::KnowExists),
            };
        }
        if !best.can_view()
            && node
                .grants
                .get(&ANYONE_INSIDE)
                .is_some_and(|open| open.can_view())
            && let Some(owner) = owner_of(graph, id)
        {
            let inside: Vec<Uuid> = graph
                .ids()
                .into_iter()
                .filter(|block| inside(graph, owner, *block))
                .collect();
            if inside
                .into_iter()
                .any(|block| level(graph, account, block, memo).can_view())
            {
                best = Access::View;
            }
        }
        memo.insert(id, best);
        best
    }
    fn in_settings(graph: &BlockGraph, block: Uuid) -> bool {
        let mut path = vec![block];
        let mut current = block;
        loop {
            let node = graph.get(current).unwrap();
            if matches!(node.parent, BlockParent::Block(_))
                && (node.content_type == SETTINGS_TYPE || node.content_type == TAGS_TYPE)
            {
                return true;
            }
            match node.parent {
                BlockParent::Block(parent) if !path.contains(&parent) && graph.contains(parent) => {
                    path.push(parent);
                    current = parent;
                }
                _ => return false,
            }
        }
    }
    fn owner_of(graph: &BlockGraph, kind: Uuid) -> Option<Uuid> {
        let held = graph.get(kind)?;
        if held.content_type == TAGS_TYPE {
            return held.parent.block();
        }
        let settings = held.parent.block()?;
        let node = graph.get(settings)?;
        match (node.content_type == SETTINGS_TYPE, node.parent) {
            (true, BlockParent::Block(owner)) => Some(owner),
            _ => None,
        }
    }
    fn inside(graph: &BlockGraph, owner: Uuid, block: Uuid) -> bool {
        let mut path = vec![block];
        let mut current = block;
        while let Some(BlockParent::Block(parent)) = graph.get(current).map(|node| node.parent) {
            if parent == owner {
                return path.iter().all(|step| {
                    let node = graph.get(*step).unwrap();
                    !((node.content_type == SETTINGS_TYPE || node.content_type == TAGS_TYPE)
                        && matches!(node.parent, BlockParent::Block(_)))
                });
            }
            if path.contains(&parent) {
                return false;
            }
            path.push(parent);
            current = parent;
        }
        false
    }
    let mut access: BTreeMap<Uuid, Access> = BTreeMap::new();
    for id in graph.ids() {
        level(graph, account, id, &mut access);
    }
    let mut referenced = Vec::new();
    for id in graph.ids() {
        if !access[&id].can_view() {
            continue;
        }
        for reference in graph.references(id) {
            if graph
                .get(reference)
                .is_some_and(|child| child.parent != BlockParent::Block(id))
            {
                referenced.push(reference);
            }
        }
    }
    for id in referenced {
        if let Some(level) = access.get_mut(&id) {
            *level = (*level).max(Access::KnowExists);
        }
    }
    access
}
