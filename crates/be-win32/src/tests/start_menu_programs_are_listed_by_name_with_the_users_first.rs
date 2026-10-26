use super::*;
use std::fs;

#[test]
fn start_menu_programs_are_listed_by_name_with_the_users_first() {
    let root = std::env::temp_dir().join(format!("be-win32-programs-{}", std::process::id()));
    let user = root.join("user");
    let everyone = root.join("everyone");
    for (folder, file) in [
        (&user, "zed.lnk"),
        (&user, "Accessories/Paint.lnk"),
        (&user, "Accessories/notes.txt"),
        (&everyone, "Accessories/paint.LNK"),
        (&everyone, "Accessories/Uninstall Paint.lnk"),
        (&everyone, "Tools/Alpha.lnk"),
        (&everyone, "Tools/Alpha copy.lnk"),
        (&everyone, "Alpha.lnk"),
        (&everyone, "Broken.lnk"),
        (&everyone, "Cleanup.lnk"),
    ] {
        let path = folder.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"").unwrap();
    }
    let programs = programs::list(&[user.clone(), everyone.clone()], |path| {
        let name = path.file_stem()?.to_string_lossy().into_owned();
        let target = match name.as_str() {
            "Broken" => return None,
            "Cleanup" => "C:\\App\\unins000.exe".to_owned(),
            "Alpha copy" | "Alpha" => "C:\\Alpha\\alpha.exe".to_owned(),
            _ => format!("C:\\{name}.exe"),
        };
        Some(Shortcut {
            target: Some(PathBuf::from(target)),
            arguments: "--start \"a b\"".to_owned(),
            ..Shortcut::default()
        })
    });
    let listed: Vec<(&str, &str, Option<&str>)> = programs
        .iter()
        .map(|program| {
            (
                program.id.as_str(),
                program.name.as_str(),
                program.folder.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("Alpha", "Alpha", None),
            ("Tools/Alpha copy", "Alpha copy", Some("Tools")),
            ("Accessories/Paint", "Paint", Some("Accessories")),
            ("zed", "zed", None),
        ]
    );
    assert_eq!(programs[2].path, user.join("Accessories/Paint.lnk"));
    assert_eq!(programs[2].command(), ["C:\\Paint.exe", "--start", "a b"]);
    fs::remove_dir_all(&root).unwrap();
}
