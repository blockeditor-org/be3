const DIFF_LIMIT: usize = 4000;

pub fn changes(before: &str, after: &str) -> String {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old = &old[prefix..old.len() - suffix];
    let new = &new[prefix..new.len() - suffix];
    if old.is_empty() && new.is_empty() {
        return "no change to the tree\n".to_owned();
    }
    let mut text = String::new();
    if old.len().saturating_mul(new.len()) > DIFF_LIMIT * DIFF_LIMIT / 4 {
        for line in old {
            text.push_str("- ");
            text.push_str(line);
            text.push('\n');
        }
        for line in new {
            text.push_str("+ ");
            text.push_str(line);
            text.push('\n');
        }
        return text;
    }
    let width = new.len() + 1;
    let mut common = vec![0u32; (old.len() + 1) * width];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            common[i * width + j] = if old[i] == new[j] {
                common[(i + 1) * width + j + 1] + 1
            } else {
                common[(i + 1) * width + j].max(common[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            i += 1;
            j += 1;
        } else if i < old.len()
            && (j == new.len() || common[(i + 1) * width + j] >= common[i * width + j + 1])
        {
            text.push_str("- ");
            text.push_str(old[i]);
            text.push('\n');
            i += 1;
        } else {
            text.push_str("+ ");
            text.push_str(new[j]);
            text.push('\n');
            j += 1;
        }
    }
    text
}
