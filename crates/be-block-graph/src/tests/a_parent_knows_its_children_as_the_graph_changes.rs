use super::*;

#[test]
fn a_parent_knows_its_children_as_the_graph_changes() {
    varied_graphs(|graph| {
        let ids = graph.ids();
        let parents = [BlockParent::Root, BlockParent::Detached]
            .into_iter()
            .chain(ids.iter().map(|block| BlockParent::Block(*block)));
        for parent in parents {
            let scanned: Vec<Uuid> = ids
                .iter()
                .copied()
                .filter(|block| graph.get(*block).unwrap().parent == parent)
                .collect();
            assert_eq!(graph.children(parent), scanned, "children of {parent:?}");
        }
        for block in &ids {
            let mut below: Vec<Uuid> = ids
                .iter()
                .copied()
                .filter(|other| other == block || graph.ancestors(*other).any(|up| up == *block))
                .collect();
            let mut subtree = graph.subtree(*block);
            assert_eq!(subtree[0], *block);
            subtree.sort_unstable();
            below.sort_unstable();
            assert_eq!(subtree, below, "subtree of {block}");
            for reference in graph.references(*block) {
                assert!(graph.backrefs(reference).contains(block));
            }
        }
        let dead: Vec<Uuid> = ids
            .iter()
            .copied()
            .filter(|block| !graph.is_live(*block))
            .collect();
        assert_eq!(graph.detached(), dead);
    });
}
