use rand::RngCore;

const F64_UNIT_INTERVAL_SCALE: f64 = 1.0 / ((1u64 << 53) as f64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RareErrorTelemetry {
    pub iterator_builds: usize,
    pub rng_core_draws: usize,
}

#[derive(Debug, Clone, Copy)]
enum RareErrorMode {
    Empty,
    Dense,
    Sparse { log_one_minus_p: f64 },
}

pub struct RareErrorIterator<'a, R: RngCore + ?Sized> {
    rng: &'a mut R,
    attempt_count: usize,
    next_candidate: usize,
    mode: RareErrorMode,
    telemetry: RareErrorTelemetry,
}

pub fn rare_error_indices<'a, R: RngCore + ?Sized>(
    probability: f64,
    attempt_count: usize,
    rng: &'a mut R,
) -> RareErrorIterator<'a, R> {
    RareErrorIterator::new(probability, attempt_count, rng)
}

impl<'a, R: RngCore + ?Sized> RareErrorIterator<'a, R> {
    pub fn new(probability: f64, attempt_count: usize, rng: &'a mut R) -> Self {
        let mode = if attempt_count == 0 || probability <= 0.0 || probability.is_nan() {
            RareErrorMode::Empty
        } else if probability >= 1.0 {
            RareErrorMode::Dense
        } else {
            RareErrorMode::Sparse {
                log_one_minus_p: (-probability).ln_1p(),
            }
        };

        Self {
            rng,
            attempt_count,
            next_candidate: 0,
            mode,
            telemetry: RareErrorTelemetry {
                iterator_builds: 1,
                rng_core_draws: 0,
            },
        }
    }

    pub fn telemetry(&self) -> RareErrorTelemetry {
        self.telemetry
    }

    fn draw_open_unit_f64(&mut self) -> f64 {
        loop {
            let raw = self.rng.next_u64();
            self.telemetry.rng_core_draws += 1;
            let value = ((raw >> 11) as f64) * F64_UNIT_INTERVAL_SCALE;
            if value > 0.0 {
                return value;
            }
        }
    }
}

impl<R: RngCore + ?Sized> Iterator for RareErrorIterator<'_, R> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match self.mode {
            RareErrorMode::Empty => None,
            RareErrorMode::Dense => {
                if self.next_candidate >= self.attempt_count {
                    return None;
                }
                let index = self.next_candidate;
                self.next_candidate += 1;
                Some(index)
            }
            RareErrorMode::Sparse { log_one_minus_p } => {
                while self.next_candidate < self.attempt_count {
                    let uniform = self.draw_open_unit_f64();
                    let skip = (uniform.ln() / log_one_minus_p).floor();
                    let skip = if skip.is_finite() && skip >= 0.0 {
                        skip as usize
                    } else {
                        usize::MAX
                    };
                    let index = self.next_candidate.saturating_add(skip);
                    if index >= self.attempt_count {
                        self.next_candidate = self.attempt_count;
                        return None;
                    }
                    self.next_candidate = index + 1;
                    return Some(index);
                }
                None
            }
        }
    }
}
