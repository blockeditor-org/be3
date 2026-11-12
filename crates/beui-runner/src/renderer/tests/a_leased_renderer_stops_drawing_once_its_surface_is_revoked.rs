use super::*;

#[test]
fn a_leased_renderer_stops_drawing_once_its_surface_is_revoked() {
    let touches = Touches::default();
    let lease = SurfaceLease::new();
    let (mut renderer, attaching) = Leased::new(Box::new(Counting(touches.clone())), lease.clone());
    let output = WindowOutput {
        id: beui_types::WindowId::MAIN,
        size: Vec2::ZERO,
        layers: std::rc::Rc::new(Vec::new()),
        window: beui_core::window::WindowState::default(),
        changed: true,
        damage: beui_types::geometry::Region::default(),
        moved: None,
        pixels_per_point: 1.0,
    };
    let draw = |renderer: &mut Leased| {
        renderer.prepare(&output, 1.0, Color32::BLACK);
        renderer.present(Color32::BLACK);
    };

    attaching.set(lease.current());
    renderer.attach(Arc::new(Window), 10, 10).unwrap();
    draw(&mut renderer);
    assert_eq!(touches.take(), ["attach", "prepare", "present"]);

    let handed = lease.current();
    std::thread::scope(|threads| {
        threads.spawn(|| lease.revoke());
    });
    draw(&mut renderer);
    renderer.resize(20, 20);
    assert_eq!(
        touches.take(),
        Vec::<&str>::new(),
        "once another thread revokes the surface, nothing reaches it"
    );

    renderer.detach();
    attaching.set(handed);
    renderer.attach(Arc::new(Window), 10, 10).unwrap();
    draw(&mut renderer);
    assert_eq!(
        touches.take(),
        Vec::<&str>::new(),
        "a surface handed over before it was revoked is not attached once it has been"
    );

    renderer.detach();
    renderer.resize(30, 30);
    attaching.set(lease.current());
    renderer.attach(Arc::new(Window), 30, 30).unwrap();
    draw(&mut renderer);
    assert_eq!(
        touches.take(),
        ["resize", "attach", "prepare", "present"],
        "the next surface is drawn to, and a detached renderer resizes freely"
    );
}
