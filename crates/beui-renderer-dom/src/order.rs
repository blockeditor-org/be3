pub fn longest_increasing(assigned: &[Option<usize>]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; assigned.len()];
    for (index, value) in assigned.iter().enumerate() {
        let Some(value) = *value else {
            continue;
        };
        let at = tails
            .partition_point(|&tail| assigned[tail].is_some_and(|candidate| candidate < value));
        if at > 0 {
            previous[index] = Some(tails[at - 1]);
        }
        match at == tails.len() {
            true => tails.push(index),
            false => tails[at] = index,
        }
    }
    let mut kept = vec![false; assigned.len()];
    let mut current = tails.last().copied();
    while let Some(index) = current {
        kept[index] = true;
        current = previous[index];
    }
    kept
}
