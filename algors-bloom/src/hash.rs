use core::convert::Infallible;
use core::fmt;
use xxhash_rust::xxh3::xxh3_128_with_secret;

/// Trait for the bloom filter's hashing implementations.
pub trait Hasher {
    type DeserializeError;
    type SerializeError;

    /// Computes independent hashes.
    fn hash_bytes<T: AsRef<[u8]>>(&self, data: T) -> (u64, u64);

    /// Serializes the hasher for persistence of state.
    fn serialize(&self) -> Result<Box<[u8]>, Self::SerializeError>;

    /// Reconstructs the hasher from a serialized exported state.
    fn from_serialized<R>(state: R) -> Result<Self, Self::DeserializeError>
    where
        R: AsRef<[u8]>,
        Self: Sized;
}

/// Minimum size for an XXH3 secret. Copied from xxhash-rust.
pub const XXH_SECRET_SIZE_MIN: usize = 136;

#[derive(Debug, Clone, Copy)]
pub enum XxHasherError {
    BufferTooShort { size: usize },
}

impl fmt::Display for XxHasherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooShort { size } => {
                write!(
                    f,
                    "buffer size {} is below the minimum required ({})",
                    size, XXH_SECRET_SIZE_MIN
                )
            }
        }
    }
}

impl core::error::Error for XxHasherError {}

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
            panic!("{}", XxHasherError::BufferTooShort { size: secret.len() });
        }
        Self { secret }
    }
}

impl Hasher for XxHasher {
    type SerializeError = Infallible;
    type DeserializeError = XxHasherError;

    #[inline(always)]
    fn hash_bytes<T: AsRef<[u8]>>(&self, data: T) -> (u64, u64) {
        let res = xxh3_128_with_secret(data.as_ref(), &self.secret);
        ((res >> 64) as u64, res as u64)
    }

    fn serialize(&self) -> Result<Box<[u8]>, Self::SerializeError> {
        Ok(self.secret.clone())
    }

    fn from_serialized<R>(state: R) -> Result<Self, Self::DeserializeError>
    where
        R: AsRef<[u8]>,
    {
        let state_copy = Vec::from(state.as_ref()).into_boxed_slice();
        if state_copy.len() < XXH_SECRET_SIZE_MIN {
            return Err(XxHasherError::BufferTooShort {
                size: state_copy.len(),
            });
        }
        Ok(Self { secret: state_copy })
    }
}
