use super::completion::{Completer, Completion};

const RESULTS: usize = 8;
const POPULAR: [&str; RESULTS] = [
    "smile", "joy", "heart", "+1", "tada", "fire", "eyes", "rocket",
];

pub fn emoji_completer() -> Completer {
    Completer::new(':', search_emoji)
}

pub fn search_emoji(query: &str) -> Vec<Completion> {
    let query = query.to_ascii_lowercase();
    if query.is_empty() {
        return POPULAR
            .iter()
            .filter_map(|code| Some(completion(emojis::get_by_shortcode(code)?, code)))
            .collect();
    }
    let mut ranked: Vec<(u8, usize, usize, Completion)> = Vec::new();
    for (order, emoji) in emojis::iter().filter(|emoji| drawable(emoji)).enumerate() {
        let best = emoji
            .shortcodes()
            .filter_map(|code| rank(code, &query).map(|rank| (rank, code)))
            .min_by_key(|(rank, code)| (*rank, code.len()));
        if let Some((rank, code)) = best {
            ranked.push((rank, code.len(), order, completion(emoji, code)));
        }
    }
    ranked.sort_by_key(|(rank, length, order, _)| (*rank, *length, *order));
    ranked
        .into_iter()
        .take(RESULTS)
        .map(|(_, _, _, completion)| completion)
        .collect()
}

fn drawable(emoji: &emojis::Emoji) -> bool {
    !emoji
        .as_str()
        .chars()
        .any(|character| matches!(character, '\u{1f1e6}'..='\u{1f1ff}' | '\u{e0020}'..='\u{e007f}'))
}

fn rank(code: &str, query: &str) -> Option<u8> {
    if code == query {
        Some(0)
    } else if code.starts_with(query) {
        Some(1)
    } else if code
        .split(['_', '-'])
        .skip(1)
        .any(|word| word.starts_with(query))
    {
        Some(2)
    } else if code.contains(query) {
        Some(3)
    } else {
        None
    }
}

fn completion(emoji: &emojis::Emoji, code: &str) -> Completion {
    Completion {
        label: format!(":{code}:"),
        insert: emoji.as_str().to_owned(),
    }
}
