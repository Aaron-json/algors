use xxhash_rust::xxh3::xxh3_128_with_secret;

/// Trait for the bloom filter's hashing implementations.
pub trait Hasher {
    /// Computes independent hashes.
    fn hash_bytes<T: AsRef<[u8]>>(&self, data: T) -> (u64, u64);
}

/// Minimum size for an XXH3 secret. Copied from xxhash-rust.
pub const XXH_SECRET_SIZE_MIN: usize = 136;

/// XXH3 implementation using a custom secret buffer.
///
/// Accepts a user generated secret to prevent collision attacks.
#[derive(Clone, Debug)]
pub struct XxHasher {
    secret: Box<[u8]>,
}

impl XxHasher {
    /// Create a new XXH3 hasher with a custom secret.
    ///
    /// # Panics
    /// Panics if the secret is shorter than the minimum SECRET_SIZE_MIN.
    pub fn new(secret: Box<[u8]>) -> Self {
        if secret.len() < XXH_SECRET_SIZE_MIN {
            panic!("XXH3 secret buffer too short");
        }
        Self { secret }
    }
}

impl Hasher for XxHasher {
    #[inline(always)]
    fn hash_bytes<T: AsRef<[u8]>>(&self, data: T) -> (u64, u64) {
        let res = xxh3_128_with_secret(data.as_ref(), &self.secret);
        ((res >> 64) as u64, res as u64)
    }
}
