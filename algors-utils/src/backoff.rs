const SPIN_MAX: u32 = 6;
const YIELD_MAX: u32 = 8;

// Utilitiy function that spins 2^(pow) times.
fn spin_for(pow: u32) {
    for _ in 0..(1 << pow) {
        core::hint::spin_loop();
    }
}

pub struct Backoff {
    count: u32,
}

impl Backoff {
    #[inline(always)]
    pub fn new() -> Self {
        Self { count: 0 }
    }

    // Implements backoff with pure spinning.
    #[inline]
    pub fn spin(&mut self) {
        spin_for(self.count.min(SPIN_MAX));
        self.count = self.count.saturating_add(1);
    }

    /// Implements backoff by spinning first, then attempts
    /// to yield execution if the `std` feature is enabled.
    /// If the feature is not enabled, it continues to spin
    /// at the bounded number of times.
    #[inline]
    pub fn pause(&mut self) {
        if self.count <= SPIN_MAX {
            spin_for(self.count);
        } else {
            #[cfg(feature = "std")]
            std::thread::yield_now();

            // If running without a stdlib, keep spinning
            // at the limit.
            #[cfg(not(feature = "std"))]
            spin_for(self.count.min(SPIN_MAX));
        }

        self.count = self.count.saturating_add(1);
    }

    /// Returns whether we should give up backoff attempts and resort to
    /// another strategy.
    #[inline(always)]
    pub fn done(&mut self) -> bool {
        return self.count >= YIELD_MAX;
    }

    /// Resets the backoff for reuse.
    #[inline(always)]
    pub fn reset(&mut self) {
        self.count = 0;
    }
}
