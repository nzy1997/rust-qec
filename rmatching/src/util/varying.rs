use std::ops::{Add, BitAnd, BitOr, Shl, Shr, Sub};

/// Trait for integer types that can be used with `Varying`.
pub trait VaryingInt:
    Copy
    + Eq
    + Ord
    + std::fmt::Debug
    + Add<Output = Self>
    + Sub<Output = Self>
    + Shr<i32, Output = Self>
    + Shl<i32, Output = Self>
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + From<i32>
{
    fn zero() -> Self;
    fn one() -> Self;
    fn two() -> Self;
    fn three() -> Self;
}

impl VaryingInt for i32 {
    #[inline]
    fn zero() -> Self {
        0
    }
    #[inline]
    fn one() -> Self {
        1
    }
    #[inline]
    fn two() -> Self {
        2
    }
    #[inline]
    fn three() -> Self {
        3
    }
}

impl VaryingInt for i64 {
    #[inline]
    fn zero() -> Self {
        0
    }
    #[inline]
    fn one() -> Self {
        1
    }
    #[inline]
    fn two() -> Self {
        2
    }
    #[inline]
    fn three() -> Self {
        3
    }
}

/// Bit-packed time-varying value.
///
/// Bottom 2 bits encode slope:
///   0b00 = frozen (slope 0)
///   0b01 = growing (slope +1)
///   0b10 = shrinking (slope -1)
///
/// Remaining bits encode y-intercept: `data >> 2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Varying<T: VaryingInt>(pub T);

pub type Varying32 = Varying<i32>;
pub type Varying64 = Varying<i64>;
/// Cumulative-time varying (used throughout the flooding algorithm).
pub type VaryingCT = Varying<i64>;

impl<T: VaryingInt> Varying<T> {
    // --- Query ---

    #[inline]
    pub fn y_intercept(&self) -> T {
        self.0 >> 2
    }

    #[inline]
    pub fn is_growing(&self) -> bool {
        (self.0 & T::one()) != T::zero()
    }

    #[inline]
    pub fn is_shrinking(&self) -> bool {
        (self.0 & T::two()) != T::zero()
    }

    #[inline]
    pub fn is_frozen(&self) -> bool {
        (self.0 & T::three()) == T::zero()
    }

    /// Evaluate the linear function at `time`.
    #[inline]
    pub fn get_distance_at_time(&self, time: T) -> T {
        if self.is_growing() {
            (self.0 >> 2) + time
        } else if self.is_shrinking() {
            (self.0 >> 2) - time
        } else {
            self.0 >> 2
        }
    }

    /// Time at which this varying reaches zero.
    pub fn time_of_x_intercept(&self) -> T {
        if self.is_growing() {
            T::zero() - (self.0 >> 2)
        } else if self.is_shrinking() {
            self.0 >> 2
        } else {
            panic!("frozen varying has no x-intercept")
        }
    }

    /// Time at which `self + other == 0`.
    pub fn time_of_x_intercept_when_added_to(&self, other: Varying<T>) -> T {
        let neg_sum = T::zero() - (self.0 >> 2) - (other.0 >> 2);
        if self.is_growing() && other.is_growing() {
            neg_sum >> 1 // combined slope = 2
        } else {
            neg_sum // combined slope = 1
        }
    }

    /// True if exactly one is growing and the other is growing or frozen
    /// (i.e. they are approaching each other).
    #[inline]
    pub fn colliding_with(&self, other: Varying<T>) -> bool {
        ((self.0 | other.0) & T::three()) == T::one()
    }

    // --- State transitions ---

    pub fn then_growing_at_time(&self, time: T) -> Varying<T> {
        Varying((self.get_distance_at_time(time) - time) << 2 | T::one())
    }

    pub fn then_shrinking_at_time(&self, time: T) -> Varying<T> {
        Varying((self.get_distance_at_time(time) + time) << 2 | T::two())
    }

    pub fn then_frozen_at_time(&self, time: T) -> Varying<T> {
        Varying(self.get_distance_at_time(time) << 2)
    }

    // --- Factory methods ---

    pub fn growing_varying_with_zero_distance_at_time(time: T) -> Varying<T> {
        Varying((T::zero() - time) << 2 | T::one())
    }

    pub fn frozen(base: T) -> Varying<T> {
        Varying(base << 2)
    }
}

// --- Arithmetic: shift y-intercept by a constant ---

impl<T: VaryingInt> Add<T> for Varying<T> {
    type Output = Varying<T>;
    #[inline]
    fn add(self, rhs: T) -> Varying<T> {
        Varying(self.0 + (rhs << 2))
    }
}

impl<T: VaryingInt> Sub<T> for Varying<T> {
    type Output = Varying<T>;
    #[inline]
    fn sub(self, rhs: T) -> Varying<T> {
        Varying(self.0 - (rhs << 2))
    }
}