const THREAD_LOCAL_SECTION: &str = ".tdata";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThreadLocals {
    pub inputs: Vec<String>,
    pub symbols: Vec<Placed>,
}

fn column(header: &str, title: &str) -> Option<usize> {
    header.find(title)
}

fn hex(token: &str) -> Option<u32> {
    u32::from_str_radix(token, 16).ok()
}

pub fn thread_locals(map: &str) -> Result<ThreadLocals, String> {
    let mut lines = map.lines();
    let header = lines.next().ok_or("the link map is empty")?;
    let (Some(out), Some(input), Some(symbol)) = (
        column(header, "Out"),
        column(header, "In "),
        column(header, "Symbol"),
    ) else {
        return Err(format!("the link map's header is not one this reads: {header}"));
    };
    let mut found = ThreadLocals::default();
    let mut inside = false;
    for line in lines {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some(last) = tokens.last() else {
            continue;
        };
        let at = line.len() - line.trim_start_matches(char::is_whitespace).len();
        let starts = line.rfind(last).unwrap_or(at);
        if starts == out {
            inside = *last == THREAD_LOCAL_SECTION;
            continue;
        }
        if !inside || tokens.len() < 4 {
            continue;
        }
        if starts == input {
            found.inputs.push(tokens[3..].join(" "));
        } else if starts == symbol {
            let (Some(offset), Some(size)) = (hex(tokens[0]), hex(tokens[2])) else {
                continue;
            };
            found.symbols.push(Placed {
                name: (*last).to_owned(),
                offset,
                size,
            });
        }
    }
    Ok(found)
}
