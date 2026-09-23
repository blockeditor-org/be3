use super::*;

#[test]
fn editing_one_item_wakes_only_the_bindings_that_read_it() {
    let client = client();
    let block = client.create_block(Presentation::default());
    for index in 0..3 {
        block.operate(add_slide(index));
    }
    let ids: Vec<Uuid> = block
        .read()
        .unwrap()
        .slides()
        .iter()
        .map(|slide| slide.id)
        .collect();

    let source = BlockSource::new(block.clone(), || {});
    let items = source.project_keyed(|presentation, items| {
        items.reconcile(presentation.slides().iter().map(|slide| (slide.id, slide)));
    });
    let done = source.project(|presentation: &Presentation| presentation.slides().len());

    let scope = Scope::new();
    let runs: Vec<Rc<Cell<usize>>> = ids.iter().map(|_| Rc::new(Cell::new(0))).collect();
    let done_runs = Rc::new(Cell::new(0));
    scope.run(|| {
        for (id, runs) in ids.iter().zip(&runs) {
            let item = items.get(id);
            let runs = runs.clone();
            create_effect(move || {
                item.with(|_| ());
                runs.set(runs.get() + 1);
            });
        }
        let counted = done_runs.clone();
        create_effect(move || {
            done.with(|_| ());
            counted.set(counted.get() + 1);
        });
    });
    for run in &runs {
        run.set(0);
    }
    done_runs.set(0);

    block.operate(PresentationOperation::SetBlockId {
        slide_id: ids[1],
        block_id: BlockRef::Direct(Uuid::new_v4()),
    });
    source.pump();

    assert_eq!(
        runs.iter().map(|run| run.get()).collect::<Vec<_>>(),
        [0, 1, 0]
    );
    assert_eq!(done_runs.get(), 0);
}
