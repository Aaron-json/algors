// Maximum spin value
const SPIN_MAX: u32 = 6;
// Maximum stems
const YIELD_MAX: u32 = 8;

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
        for _ in 0..(1 << self.count.min(SPIN_MAX)) {
            std::hint::spin_loop();
        }

        self.count = self.count.saturating_add(1);
    }

    // Implements backoff by spinning first, then yielding.
    #[inline]
    pub fn pause(&mut self) {
        if self.count <= SPIN_MAX {
            for _ in 0..(1 << self.count) {
                std::hint::spin_loop();
            }
        } else {
            std::thread::yield_now();
        }

        self.count = self.count.saturating_add(1);
    }

    // Returns whether we should give up backoff attempts and resort to
    // another strategy.
    #[inline(always)]
    pub fn done(&mut self) -> bool {
        return self.count >= YIELD_MAX;
    }

    // Resets the backoff for reuse.
    #[inline(always)]
    pub fn reset(&mut self) {
        self.count = 0;
    }
}
