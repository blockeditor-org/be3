use super::*;

#[test]
fn a_field_left_at_its_default_is_not_written() {
    #[derive(Serialize)]
    struct Before {
        clip: [f32; 4],
        content: Content,
    }
    let content = Content::Callback([1.0, 2.0, 3.0, 4.0]);
    let before = encode(&Before {
        clip: [0.0, 0.0, 8.0, 8.0],
        content: content.clone(),
    })
    .unwrap();
    let primitive = Primitive {
        clip: [0.0, 0.0, 8.0, 8.0],
        fade: Fade::NONE,
        content,
    };

    assert_eq!(encode(&primitive).unwrap(), before);
    assert_eq!(decode::<Primitive>(&before).unwrap(), primitive);

    let faded = Primitive {
        fade: Fade {
            rect: [0.0, 0.0, 8.0, 8.0],
            widths: [0.0, 2.0, 0.0, 2.0],
            opacity: 1.0,
        },
        ..primitive
    };
    assert_eq!(
        decode::<Primitive>(&encode(&faded).unwrap()).unwrap(),
        faded
    );
}
