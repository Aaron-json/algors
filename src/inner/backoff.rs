// Maximum spin value
const SPIN_MAX: u32 = 6;
// Maximum stems
const PAUSE_MAX: u32 = 8;

pub struct Backoff {
    count: u32,
}

impl Backoff {
    #[inline]
    pub fn new() -> Self {
        Self { count: 0 }
    }

    // Implements a spin loop with exponential backoff.
    // Does not yield/park
    //
    // Useful when retrying a failed operation.
    #[inline]
    pub fn spin(&mut self) {
        for _ in 0..(1 << self.count.min(SPIN_MAX)) {
            std::hint::spin_loop();
        }

        if self.count <= SPIN_MAX {
            self.count += 1;
        }
    }

    // Implements backoff with potential blocking.
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

    // Returns if Backoff has maximized its counter limits
    // and the user should probably implement blocking or other
    // more appropriate solutions.
    pub fn saturated(&mut self) -> bool {
        return self.count >= PAUSE_MAX;
    }
}
