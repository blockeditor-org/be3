use super::*;

#[test]
fn numbers_are_fixed_width_and_their_lists_are_their_memory() {
    assert_eq!(bytes(&5u64), 5u64.to_le_bytes());
    assert_eq!(bytes(&-2i16), (-2i16).to_le_bytes());
    assert_eq!(bytes(&7u8), [7]);
    assert_eq!(bytes(&-1i32), [0xff; 4]);

    let numbers: Vec<u16> = (0..500).map(|i| i * 131).collect();
    let encoded = bytes(&numbers);
    // The numbers' type, then their memory.
    assert_eq!(encoded[0], Number::U16 as u8);
    assert_eq!(encoded.len(), 1 + numbers.len() * 2);
    let wide: Vec<i64> = vec![-1, 0, i64::MAX, 3];
    let floats: Vec<f64> = vec![1.5, -0.0, f64::INFINITY];
    if cfg!(target_endian = "little") {
        assert_eq!(&encoded[1..], bytemuck::cast_slice::<u16, u8>(&numbers));
        assert_eq!(&bytes(&wide)[1..], bytemuck::cast_slice::<i64, u8>(&wide));
        assert_eq!(
            &bytes(&floats)[1..],
            bytemuck::cast_slice::<f64, u8>(&floats)
        );
    }
    assert_eq!(decode::<Vec<u16>>(&encoded), Ok(numbers));
    assert_eq!(decode::<Vec<i64>>(&bytes(&wide)), Ok(wide));
    assert_eq!(decode::<Vec<f64>>(&bytes(&floats)), Ok(floats));

    // A list of one number type read as another is refused, never read as garbage.
    assert_eq!(
        kind(decode::<Vec<u32>>(&bytes(&vec![1u64]))),
        ErrorKind::Number {
            expected: Number::U32,
            found: Number::U64
        }
    );

    // A list holding NaN still writes every NaN alike.
    let odd_nan = f32::from_bits(0x7fc0_0001);
    assert_eq!(bytes(&vec![1.0, odd_nan]), bytes(&vec![1.0, f32::NAN]));

    assert_eq!(
        decode::<Vec<bool>>(&bytes(&vec![true, false])),
        Ok(vec![true, false])
    );
    assert_eq!(
        kind(decode::<Vec<bool>>(&[Number::Bool as u8, 1, 2])),
        ErrorKind::OutOfRange
    );
}
