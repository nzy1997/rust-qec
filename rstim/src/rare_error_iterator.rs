use rand::RngCore;
#[cfg(debug_assertions)]
use std::cell::Cell;

const F64_UNIT_INTERVAL_SCALE: f64 = 1.0 / ((1u64 << 53) as f64);

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RareErrorTelemetry {
    pub iterator_builds: usize,
    pub rng_core_draws: usize,
}

#[cfg(debug_assertions)]
thread_local! {
    static ITERATOR_BUILDS: Cell<usize> = const { Cell::new(0) };
    static RNG_CORE_DRAWS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn reset_rare_error_telemetry() {
    ITERATOR_BUILDS.with(|builds| builds.set(0));
    RNG_CORE_DRAWS.with(|draws| draws.set(0));
}

#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn rare_error_telemetry() -> RareErrorTelemetry {
    RareErrorTelemetry {
        iterator_builds: ITERATOR_BUILDS.with(Cell::get),
        rng_core_draws: RNG_CORE_DRAWS.with(Cell::get),
    }
}

#[cfg(debug_assertions)]
fn record_iterator_build() {
    ITERATOR_BUILDS.with(|builds| builds.set(builds.get().saturating_add(1)));
}

#[cfg(debug_assertions)]
fn record_rng_core_draw() {
    RNG_CORE_DRAWS.with(|draws| draws.set(draws.get().saturating_add(1)));
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
        #[cfg(debug_assertions)]
        record_iterator_build();

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
        }
    }

    fn draw_open_unit_f64(&mut self) -> f64 {
        loop {
            #[cfg(debug_assertions)]
            record_rng_core_draw();
            let raw = self.rng.next_u64();
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
