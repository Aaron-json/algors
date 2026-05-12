use crate::hash::Hasher;
use core::mem;

type BitsType = u64;

/// This implementation of the bloom filter uses ideas from the paper at
/// `https://doi.org/10.1002/rsa.20208 Digital Object Identifier (DOI)``
/// by Adam Kirsch and Michael Mitzenmacher.
///
/// # Serialization
/// The serialization format is as below
/// [16 bytes] magic
/// [1 byte]  version         
/// [4 bytes] hash_num: u32 LE
/// [4 bytes] hasher_len: u32 LE
/// [N bytes] hasher_data
/// [8 bytes] bits_num: u64 LE
/// [M bytes] bits_data
pub struct Bloom<T: Hasher> {
    bits: Box<[BitsType]>,
    // cached to avoid recomputation
    bits_num: u64,
    hash_num: u32,
    hasher: T,
}

#[derive(Debug)]
pub enum DeserializeError<T: Hasher> {
    InvalidMagic,
    InvalidVersion,
    HasherError(T::DeserializeError),
    BufferTooShort { need: usize, have: usize },
}

const MAGIC: &[u8; 16] = b"BloomFile\x00\x00\x00\x00\x00\x00\x00";
const VERSION: u8 = 1;

impl<T: Hasher> Bloom<T> {
    /// Rounds float to the next multiple of the `BitsType` bit width.
    #[inline(always)]
    fn roundf_to_bits_type_size(num: f64) -> f64 {
        (num / BitsType::BITS as f64).ceil() * BitsType::BITS as f64
    }

    /// Rounds integer to the next multiple of the `BitsType` bit width.
    ///
    /// This function uses a trick for powers of 2. Unsigned integer bit sizes
    /// are almost always powers of 2.
    #[inline(always)]
    fn roundi_to_bits_type_size(num: u64) -> u64 {
        (num + (BitsType::BITS - 1) as u64) & !(BitsType::BITS as u64 - 1)
    }

    /// Calculates the optimal size of the bit field given the size hint and
    /// the target false positive probability.
    ///
    /// # Panic
    /// Panics if `p_false` is not in the range `(0.0, 1.0)`.
    /// Panics if `size_hint` <= 0.
    fn calc_size(size_hint: usize, p_false: f64) -> u64 {
        assert!(p_false > 0.0 && p_false < 1.0);
        assert!(size_hint > 0);

        // m = - (n ln (p)) / (ln(2))^2
        const LN2_SQUARED: f64 = core::f64::consts::LN_2 * core::f64::consts::LN_2;
        let m = (-((size_hint as f64) * p_false.ln()) / LN2_SQUARED).ceil();

        assert!(m.is_finite());
        assert!(m >= 0.0);
        assert!(m <= u64::MAX as f64);

        m as u64
    }

    /// Helper to calculate the number of hash "functions" needed given
    /// the number of bits and a size hint.
    /// # Panic
    /// Panics if the resulting number of hashes cannot fit in a `u32`.
    fn calc_n_hashes(bits: u64, size_hint: usize) -> u32 {
        // k = (m/n) * (ln(2))

        // We round since the paper recommends using a slightly lower than
        // optimal k value to reduce hashing compute.
        let k = ((bits as f64 / size_hint as f64) * core::f64::consts::LN_2)
            .floor()
            .max(1.0);

        assert!(k.is_finite());
        assert!(k <= u32::MAX as f64);

        k as u32
    }

    /// Create a new `Bloom` object with the target parameters.
    /// # Panic
    /// Panics if the allocation length cannot fit in a `usize`.
    pub fn with_hints(size_hint: usize, p_false: f64, hasher: T) -> Self {
        let bit_size = Self::calc_size(size_hint, p_false);
        let bits_rounded = Self::roundi_to_bits_type_size(bit_size);

        // calculate number of hashes after rounding up `BitsType`::BITS to
        // use all of the allocated space. This gives us a bigger space
        // and may reduce the number of hashes needed.
        let n_hashes = Self::calc_n_hashes(bits_rounded, size_hint);

        let length = bits_rounded >> BitsType::BITS.trailing_zeros();
        assert!(length <= usize::MAX as u64);
        let bits = vec![0; length as usize].into_boxed_slice();

        Bloom {
            bits,
            bits_num: bits_rounded,
            hash_num: n_hashes,
            hasher,
        }
    }

    /// Computes a 128-bit hash for the given data.
    fn hash_bytes<R>(&self, data: R) -> (u64, u64)
    where
        R: AsRef<[u8]>,
    {
        let (h1, mut h2) = self.hasher.hash_bytes(data);

        // We ensure that h2 is odd so it is coprime with 2^BitsType::BITS.
        // This avoids strides (h2) that "skip" parts of the bit space to
        // create a smaller possible output space for an input.
        h2 = (h2 as u64) | 0x1;

        (h1, h2)
    }

    /// Helper function to calculate the bit index given the hash and the
    /// index of the hash function.
    #[inline(always)]
    fn bit_index(&self, h1: u64, h2: u64, i: u32) -> u64 {
        let h_combined = h1.wrapping_add((i as u64).wrapping_mul(h2));

        // fastrange to avoid modulo
        ((h_combined as u128 * self.bits_num as u128) >> 64) as u64
    }

    /// Returns whether the data PROBABLY exists in the set or if it
    /// definitely does not exist.
    pub fn contains<R>(&self, data: R) -> bool
    where
        R: AsRef<[u8]>,
    {
        let data_ref = data.as_ref();
        let (h1, h2) = self.hash_bytes(data_ref);

        for i in 0..self.hash_num {
            let bit_idx = self.bit_index(h1, h2, i);

            let elem_idx = bit_idx >> BitsType::BITS.trailing_zeros();
            let mask = 1 << (bit_idx & (BitsType::BITS as u64 - 1));

            if self.bits[elem_idx as usize] & mask == 0 {
                return false;
            }
        }

        return true;
    }

    /// Adds the data to the set. An element can not be removed after
    /// being added.
    pub fn insert<R>(&mut self, data: R)
    where
        R: AsRef<[u8]>,
    {
        let data_ref = data.as_ref();
        let (h1, h2) = self.hash_bytes(data_ref);

        for i in 0..self.hash_num {
            let bit_idx = self.bit_index(h1, h2, i);

            let elem_idx = bit_idx >> BitsType::BITS.trailing_zeros();
            let mask = 1 << (bit_idx & (BitsType::BITS as u64 - 1));

            self.bits[elem_idx as usize] |= mask;
        }
    }

    /// Returns the number of hash functions being used.
    pub fn hash_num(&self) -> u32 {
        self.hash_num
    }

    /// Returns the total number of bits in the filter.
    pub fn bits_num(&self) -> u64 {
        self.bits_num
    }

    pub fn serialize(&self) -> Result<Box<[u8]>, T::SerializeError> {
        let hasher_bytes = self.hasher.serialize()?;
        assert!(
            hasher_bytes.len() as u64 <= u32::MAX as u64,
            "Serialized hasher must be <= u32::MAX"
        );

        let mut buf = Vec::new();

        buf.extend_from_slice(MAGIC);
        buf.push(VERSION);

        // hasher
        buf.extend_from_slice(&self.hash_num.to_le_bytes());
        buf.extend_from_slice(&(hasher_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(&hasher_bytes);

        // data
        buf.extend_from_slice(&self.bits_num.to_le_bytes());

        for &word in self.bits.iter() {
            buf.extend_from_slice(&word.to_le_bytes());
        }

        Ok(buf.into_boxed_slice())
    }

    pub fn deserialize<R>(serialized: R) -> Result<Self, DeserializeError<T>>
    where
        R: AsRef<[u8]>,
    {
        let buf = serialized.as_ref();
        let mut pos = 0;

        macro_rules! read {
            ($n:expr) => {{
                let end = pos + $n;
                if end > buf.len() {
                    return Err(DeserializeError::BufferTooShort {
                        need: end,
                        have: buf.len(),
                    });
                }
                let slice = &buf[pos..end];
                pos = end;
                slice
            }};
        }

        if read!(MAGIC.len()) != MAGIC {
            return Err(DeserializeError::InvalidMagic);
        }

        if read!(1)[0] != VERSION {
            return Err(DeserializeError::InvalidVersion);
        }

        // hasher
        let hasher_num = u32::from_le_bytes(read!(4).try_into().unwrap());
        let hasher_bytes_len = u32::from_le_bytes(read!(4).try_into().unwrap());
        let hasher_bytes = read!(hasher_bytes_len as usize);
        let hasher =
            T::from_serialized(hasher_bytes).map_err(|e| DeserializeError::HasherError(e))?;

        // bits
        let bits_num = u64::from_le_bytes(read!(8).try_into().unwrap());

        let elem_size = mem::size_of::<BitsType>();
        let remaining = buf.len() - pos;
        if remaining % elem_size != 0 {
            return Err(DeserializeError::BufferTooShort {
                // nearest valid length. original could have had more
                need: pos + elem_size - (remaining % elem_size),
                have: buf.len(),
            });
        }

        let mut bits_buf: Vec<BitsType> = vec![0; remaining / elem_size];
        for word in bits_buf.iter_mut() {
            *word = u64::from_le_bytes(read!(elem_size).try_into().unwrap());
        }

        Ok(Bloom {
            bits: bits_buf.into_boxed_slice(),
            bits_num,
            hash_num: hasher_num,
            hasher,
        })
    }
}
