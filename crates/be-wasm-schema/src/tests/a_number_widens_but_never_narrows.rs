use super::*;

#[test]
fn a_number_widens_but_never_narrows() {
    // Fields widened from u32 to u64, f32 to f64 and a list of i16 to i64 still read what was
    // written before.
    let narrow = Narrow {
        count: 5,
        ratio: 1.5,
        offsets: vec![-1, 2],
    };
    assert_eq!(
        decode::<Wide>(&bytes(&narrow)),
        Ok(Wide {
            count: 5,
            ratio: 1.5,
            offsets: vec![-1, 2]
        })
    );
    assert_eq!(decode::<Vec<u128>>(&bytes(&vec![7u32])), Ok(vec![7]));
    // An older build reading a widened field refuses it rather than misreading it.
    let wide = Wide {
        count: 5,
        ratio: 0.0,
        offsets: Vec::new(),
    };
    assert_eq!(
        kind(decode::<Narrow>(&bytes(&wide))),
        ErrorKind::Wire {
            expected: Wire::Fixed32,
            found: Wire::Fixed64
        }
    );
}
