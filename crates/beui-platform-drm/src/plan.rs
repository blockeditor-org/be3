use crate::display::{DisplayConfig, DisplayUse};

pub fn plan(ids: &[&str], config: &DisplayConfig) -> Vec<(usize, DisplayUse)> {
    let wanted: Vec<DisplayUse> = ids.iter().map(|id| config.used(id)).collect();
    let mut desktop: Vec<bool> = wanted
        .iter()
        .map(|used| *used == DisplayUse::Desktop)
        .collect();
    if !desktop.contains(&true) {
        let main = config
            .main
            .as_deref()
            .and_then(|main| ids.iter().position(|id| *id == main));
        if let Some(lit) = main.or((!ids.is_empty()).then_some(0)) {
            desktop[lit] = true;
        }
    }
    let uses: Vec<DisplayUse> = wanted
        .into_iter()
        .zip(&desktop)
        .map(|(used, desktop)| match (used, desktop) {
            (_, true) => DisplayUse::Desktop,
            (DisplayUse::Mirror(of), false) => {
                match ids.iter().position(|id| *id == of) {
                    Some(source) if desktop_at(&desktop, source) => DisplayUse::Mirror(of),
                    _ => DisplayUse::Desktop,
                }
            }
            (used, false) => used,
        })
        .collect();
    let main = config
        .main
        .as_deref()
        .and_then(|main| ids.iter().position(|id| *id == main))
        .filter(|main| uses[*main] == DisplayUse::Desktop)
        .or_else(|| uses.iter().position(|used| *used == DisplayUse::Desktop));
    let rank = |index: usize| match &uses[index] {
        _ if Some(index) == main => 0,
        DisplayUse::Desktop => 1,
        DisplayUse::Mirror(_) => 2,
        DisplayUse::Off => 3,
    };
    let mut order: Vec<usize> = (0..ids.len()).collect();
    order.sort_by_key(|index| rank(*index));
    order
        .into_iter()
        .map(|index| (index, uses[index].clone()))
        .collect()
}

fn desktop_at(desktop: &[bool], index: usize) -> bool {
    desktop.get(index).copied().unwrap_or(false)
}

#[cfg(test)]
mod tests;
