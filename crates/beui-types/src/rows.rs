pub struct RowMetrics {
    count: usize,
    estimated: f32,
    lengths: Vec<f32>,
    counts: Vec<u32>,
    sums: Vec<f64>,
    measured: f32,
    measured_rows: usize,
}

impl RowMetrics {
    pub fn new(count: usize, estimated: f32) -> Self {
        Self::measured(count, estimated, Vec::new())
    }

    pub fn measured(count: usize, estimated: f32, entries: Vec<(usize, f32)>) -> Self {
        let mut metrics = Self {
            count,
            estimated: estimated.max(0.0),
            lengths: vec![f32::NAN; count],
            counts: vec![0; count + 1],
            sums: vec![0.0; count + 1],
            measured: 0.0,
            measured_rows: 0,
        };
        for (index, length) in entries {
            metrics.record(index, length);
        }
        metrics
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn estimated(&self) -> f32 {
        self.estimated
    }

    pub fn length(&self, index: usize) -> Option<f32> {
        self.lengths
            .get(index)
            .copied()
            .filter(|length| !length.is_nan())
    }

    pub fn record(&mut self, index: usize, length: f32) -> bool {
        if index >= self.count {
            return false;
        }
        let length = length.max(0.0);
        let (added, change) = match self.length(index) {
            Some(stored) if stored == length => return false,
            Some(stored) => {
                self.measured += length - stored;
                (0, f64::from(length) - f64::from(stored))
            }
            None => {
                self.measured += length;
                self.measured_rows += 1;
                (1, f64::from(length))
            }
        };
        self.lengths[index] = length;
        let mut at = index + 1;
        while at <= self.count {
            self.counts[at] += added;
            self.sums[at] += change;
            at += at.isolate_lowest_one();
        }
        true
    }

    pub fn guess(&self) -> f32 {
        match self.measured_rows {
            0 => self.estimated,
            measured => self.measured / measured as f32,
        }
    }

    pub fn height(&self, index: usize) -> f32 {
        self.length(index).unwrap_or_else(|| self.guess())
    }

    pub fn position(&self, index: usize) -> f32 {
        let (mut measured, mut sum) = (0_usize, 0.0_f64);
        let mut at = index.min(self.count);
        while at > 0 {
            measured += self.counts[at] as usize;
            sum += self.sums[at];
            at &= at - 1;
        }
        (index.min(self.count) - measured) as f32 * self.guess() + sum as f32
    }

    pub fn total(&self) -> f32 {
        (self.count - self.measured_rows) as f32 * self.guess() + self.measured
    }

    pub fn index_at(&self, position: f32) -> usize {
        if self.count == 0 {
            return 0;
        }
        let (mut low, mut high) = (0, self.count - 1);
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            match self.position(mid) <= position {
                true => low = mid,
                false => high = mid - 1,
            }
        }
        low
    }
}
