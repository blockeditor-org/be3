use super::*;

#[test]
fn numbers_are_fixed_width_and_their_lists_are_their_memory() {
    assert_eq!(encode(&5u64), 5u64.to_le_bytes());
    assert_eq!(encode(&-2i16), (-2i16).to_le_bytes());
    assert_eq!(encode(&7u8), [7]);

    let numbers: Vec<u16> = (0..500).map(|i| i * 131).collect();
    let bytes = encode(&numbers);
    assert_eq!(bytes.len(), numbers.len() * 2);
    let wide: Vec<i64> = vec![-1, 0, i64::MAX, 3];
    let floats: Vec<f64> = vec![1.5, -0.0, f64::INFINITY];
    if cfg!(target_endian = "little") {
        assert_eq!(bytes, bytemuck::cast_slice::<u16, u8>(&numbers));
        assert_eq!(encode(&wide), bytemuck::cast_slice::<i64, u8>(&wide));
        assert_eq!(encode(&floats), bytemuck::cast_slice::<f64, u8>(&floats));
    }
    assert_eq!(decode::<Vec<u16>>(&bytes), Ok(numbers));
    assert_eq!(decode::<Vec<i64>>(&encode(&wide)), Ok(wide));
    assert_eq!(decode::<Vec<f64>>(&encode(&floats)), Ok(floats));

    // A list holding NaN still writes every NaN alike.
    let odd_nan = f32::from_bits(0x7fc0_0001);
    assert_eq!(encode(&vec![1.0, odd_nan]), encode(&vec![1.0, f32::NAN]));

    assert_eq!(encode(&vec![true, false]), [1, 0]);
    assert_eq!(decode::<Vec<bool>>(&[1, 0]), Ok(vec![true, false]));
    assert_eq!(decode::<Vec<bool>>(&[1, 2]), Err(Error::OutOfRange));
}
