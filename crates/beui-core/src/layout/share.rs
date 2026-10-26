use crate::pixel_grid::PixelGrid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub weight: f32,
    pub min: f32,
    pub max: f32,
}

impl Part {
    pub fn new(weight: f32, min: f32, max: f32) -> Self {
        let weight = if weight.is_finite() {
            weight.max(0.0)
        } else {
            0.0
        };
        let min = min.max(0.0);
        let max = match weight > 0.0 {
            true => max.max(min),
            false => min,
        };
        Self { weight, min, max }
    }

    fn at(self, rate: f32) -> f32 {
        (rate * self.weight).clamp(self.min, self.max)
    }

    fn breaks(self) -> [f32; 2] {
        [self.min / self.weight, self.max / self.weight]
    }
}

pub fn share(total: f32, parts: &[Part]) -> Vec<f32> {
    let low: f32 = parts.iter().map(|part| part.min).sum();
    if total.is_nan() || total <= low {
        return parts.iter().map(|part| part.min).collect();
    }
    let high: f32 = parts.iter().map(|part| part.max).sum();
    if total >= high {
        return parts.iter().map(|part| part.max).collect();
    }
    let rate = rate_for(total, parts);
    parts.iter().map(|part| part.at(rate)).collect()
}

fn rate_for(total: f32, parts: &[Part]) -> f32 {
    let mut fixed = 0.0f32;
    let mut slope = 0.0f32;
    let mut left = 0.0f32;
    let mut right = f32::INFINITY;
    let mut open: Vec<Part> = parts.to_vec();
    let mut breaks: Vec<f32> = Vec::with_capacity(parts.len() * 2);
    loop {
        open.retain(|part| {
            if part.weight <= 0.0 {
                fixed += part.min;
                return false;
            }
            let [start, end] = part.breaks();
            if end <= left {
                fixed += part.max;
                false
            } else if start >= right {
                fixed += part.min;
                false
            } else if start <= left && end >= right {
                slope += part.weight;
                false
            } else {
                true
            }
        });
        breaks.clear();
        breaks.extend(
            open.iter()
                .flat_map(|part| part.breaks())
                .filter(|at| *at > left && *at < right),
        );
        if breaks.is_empty() {
            break;
        }
        let middle = breaks.len() / 2;
        let (_, pivot, _) = breaks.select_nth_unstable_by(middle, f32::total_cmp);
        let pivot = *pivot;
        let reached = fixed + slope * pivot + open.iter().map(|part| part.at(pivot)).sum::<f32>();
        match reached <= total {
            true => left = pivot,
            false => right = pivot,
        }
    }
    match slope > 0.0 {
        true => ((total - fixed) / slope).clamp(left, right),
        false => left,
    }
}

pub fn snapped_run(grid: PixelGrid, lengths: &[f32]) -> Vec<f32> {
    let mut exact = 0.0f32;
    let mut placed = 0.0f32;
    lengths
        .iter()
        .map(|length| {
            exact += length;
            let edge = grid.snap(exact);
            let snapped = edge - placed;
            placed = edge;
            snapped
        })
        .collect()
}

#[cfg(test)]
mod tests;
