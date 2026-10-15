use paint_snapshot::{Content, Snapshot};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let snapshot = Snapshot::decode(&std::fs::read(path).unwrap()).unwrap();
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
            };
            println!("clip {:?} {what}", p.clip);
        }
    }
}
