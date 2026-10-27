use super::*;

#[test]
fn sharing_matches_a_step_by_step_fill_across_many_parts() {
    let parts: Vec<Part> = (0..200)
        .map(|index| {
            let weight = (index % 7) as f32;
            let min = (index % 5) as f32 * 3.0;
            let max = min + (index % 11) as f32 * 4.0 + 1.0;
            Part::new(weight, min, max)
        })
        .collect();
    let low: f32 = parts.iter().map(|part| part.min).sum();
    let high: f32 = parts.iter().map(|part| part.max).sum();

    for step in 0..=20 {
        let total = low + (high - low) * step as f32 / 20.0;
        let shares = share(total, &parts);
        let expected = filled_by_steps(total, &parts);
        let sum: f32 = shares.iter().sum();
        assert!(
            (sum - total).abs() < 0.05,
            "shares sum to {sum}, not {total}"
        );
        for (got, want) in shares.iter().zip(&expected) {
            assert!((got - want).abs() < 0.05, "{got} is not {want} at {total}");
        }
    }
}

fn filled_by_steps(total: f32, parts: &[Part]) -> Vec<f32> {
    let mut low = 0.0f32;
    let mut high = 1.0f32;
    let at = |rate: f32| parts.iter().map(|part| part.at(rate)).sum::<f32>();
    while at(high) < total && high < 1.0e9 {
        high *= 2.0;
    }
    for _ in 0..200 {
        let middle = (low + high) / 2.0;
        match at(middle) < total {
            true => low = middle,
            false => high = middle,
        }
    }
    parts.iter().map(|part| part.at(high)).collect()
}
