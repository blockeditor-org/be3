use crate::{DecodeError, MAX_MENU_DEPTH, collection};

pub(crate) fn nest<T, N>(
    items: Vec<T>,
    depth: impl Fn(&T) -> u32,
    mut node: impl FnMut(T, Vec<N>) -> Result<N, DecodeError>,
) -> Result<Vec<N>, DecodeError> {
    collection(items.len())?;
    let mut open: Vec<(Option<T>, Vec<N>)> = vec![(None, Vec::new())];
    for item in items {
        let depth = depth(&item) as usize;
        if depth > MAX_MENU_DEPTH {
            return Err(DecodeError::LimitExceeded("menu depth"));
        }
        if depth >= open.len() {
            return Err(DecodeError::MalformedPayload);
        }
        while open.len() > depth + 1 {
            close(&mut open, &mut node)?;
        }
        open.push((Some(item), Vec::new()));
    }
    while open.len() > 1 {
        close(&mut open, &mut node)?;
    }
    Ok(open.pop().map(|(_, nodes)| nodes).unwrap_or_default())
}

fn close<T, N>(
    open: &mut Vec<(Option<T>, Vec<N>)>,
    node: &mut impl FnMut(T, Vec<N>) -> Result<N, DecodeError>,
) -> Result<(), DecodeError> {
    let Some((Some(item), children)) = open.pop() else {
        return Ok(());
    };
    let made = node(item, children)?;
    if let Some((_, parent)) = open.last_mut() {
        parent.push(made);
    }
    Ok(())
}

pub(crate) fn flatten<N, T>(
    nodes: &[N],
    children: impl Fn(&N) -> &[N],
    item: impl Fn(&N, u32) -> T,
) -> Vec<T> {
    fn into<N, T>(
        nodes: &[N],
        depth: u32,
        children: &impl Fn(&N) -> &[N],
        item: &impl Fn(&N, u32) -> T,
        items: &mut Vec<T>,
    ) {
        for node in nodes {
            items.push(item(node, depth));
            into(children(node), depth + 1, children, item, items);
        }
    }
    let mut items = Vec::new();
    into(nodes, 0, &children, &item, &mut items);
    items
}
