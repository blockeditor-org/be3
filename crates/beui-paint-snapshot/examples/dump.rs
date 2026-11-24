use beui_paint_snapshot::{Content, Painting};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(path).unwrap();
    let snapshot = match Painting::decode(&bytes).unwrap() {
        Painting::Native(snapshot) => snapshot,
        Painting::Browser(_) => {
            print!("{}", std::str::from_utf8(&bytes).unwrap());
            return;
        }
    };
    for frame in &snapshot.frames {
        println!("frame {:?} ppp {}", frame.size, frame.pixels_per_point);
        for p in &frame.primitives {
            let what = match &p.content {
                Content::Mesh(t) => format!("mesh {} tris", t.len()),
                Content::Callback(r) => format!("callback {r:?}"),
                Content::RoundedRect(r) => format!(
                    "rect {:?} r{} s{} {:?}",
                    r.rect, r.corner_radius, r.stroke_width, r.color
                ),
                Content::CorneredRect(r, c) => format!("crect {:?} {:?} {:?}", r.rect, c, r.color),
                Content::Glyph(g) => format!("glyph {:?}", g.rect),
                Content::Group(g) => format!("group of {} at {}", g.primitives.len(), g.opacity),
            };
            println!("clip {:?} {what}", p.clip);
        }
    }
}
