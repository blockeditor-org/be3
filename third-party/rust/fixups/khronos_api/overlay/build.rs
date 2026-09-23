// khronos_api's build script, with the extensions copied into OUT_DIR rather
// than included from where the build script ran; the fixup says why.
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let copies = Path::new(&out_dir).join("webgl_extensions");
    fs::create_dir_all(&copies).unwrap();
    let mut file = fs::File::create(Path::new(&out_dir).join("webgl_exts.rs")).unwrap();
    let root = env::current_dir().unwrap().join("api_webgl/extensions");

    let mut names: Vec<String> = root
        .read_dir()
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir() && path.join("extension.xml").is_file())
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .filter(|name| name != "template")
        .collect();
    names.sort();

    writeln!(file, "&[").unwrap();
    for name in names {
        fs::copy(root.join(&name).join("extension.xml"), copies.join(format!("{name}.xml"))).unwrap();
        writeln!(
            file,
            "&*include_bytes!(concat!(env!(\"OUT_DIR\"), \"/webgl_extensions/{name}.xml\")),"
        )
        .unwrap();
    }
    writeln!(file, "]").unwrap();
}
