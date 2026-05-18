use algors_bloom::bloom::{Bloom, DeserializeError};
use algors_bloom::hash::{XXH_SECRET_SIZE_MIN, XxHasher};

fn test_hasher() -> XxHasher {
    let secret = vec![0x42u8; XXH_SECRET_SIZE_MIN].into_boxed_slice();
    XxHasher::new(secret)
}

#[test]
fn test_serialization_round_trip() {
    let mut bloom = Bloom::with_hints(1000, 0.01, test_hasher());
    let items: Vec<&[u8]> = vec![b"apple", b"banana", b"cherry", b"date", b"elderberry"];

    for item in &items {
        bloom.insert(item);
    }

    let serialized = bloom.serialize().expect("serialization failed");
    let deserialized: Bloom<XxHasher> =
        Bloom::deserialize(&serialized).expect("deserialization failed");

    assert_eq!(bloom.bits_num(), deserialized.bits_num());
    assert_eq!(bloom.hash_num(), deserialized.hash_num());

    for item in &items {
        assert!(deserialized.contains(item), "missing item: {:?}", item);
    }

    assert!(!deserialized.contains(b"foobar"));
}

#[test]
fn test_serialization_empty() {
    let bloom = Bloom::with_hints(100, 0.1, test_hasher());
    let serialized = bloom.serialize().expect("serialization failed");
    let deserialized: Bloom<XxHasher> =
        Bloom::deserialize(&serialized).expect("deserialization failed");

    assert_eq!(bloom.bits_num(), deserialized.bits_num());
    assert_eq!(bloom.hash_num(), deserialized.hash_num());
    assert!(!deserialized.contains(b"any"));
}

#[test]
fn test_deserialize_invalid_magic() {
    let bloom = Bloom::with_hints(100, 0.1, test_hasher());
    let mut serialized = bloom.serialize().expect("serialization failed");

    // Corrupt magic
    serialized[0] = b'X';

    let result: Result<Bloom<XxHasher>, _> = Bloom::deserialize(&serialized);
    match result {
        Err(DeserializeError::InvalidMagic) => {}
        _ => panic!("Expected InvalidMagic error, got {:?}", result),
    }
}

#[test]
fn test_deserialize_invalid_version() {
    let bloom = Bloom::with_hints(100, 0.1, test_hasher());
    let mut serialized = bloom.serialize().expect("serialization failed");

    // Magic is 16 bytes, version is at index 16
    serialized[16] = 255;

    let result: Result<Bloom<XxHasher>, _> = Bloom::deserialize(&serialized);
    match result {
        Err(DeserializeError::InvalidVersion) => {}
        _ => panic!("Expected InvalidVersion error, got {:?}", result),
    }
}

#[test]
fn test_deserialize_truncated_buffer() {
    let bloom = Bloom::with_hints(100, 0.1, test_hasher());
    let serialized = bloom.serialize().expect("serialization failed");

    for i in 0..serialized.len() - 1 {
        let truncated = &serialized[..i];
        let result: Result<Bloom<XxHasher>, _> = Bloom::deserialize(truncated);
        match result {
            Err(DeserializeError::BufferTooShort { .. }) => {}
            _ => panic!(
                "Expected BufferTooShort at truncation {}, got {:?}",
                i, result
            ),
        }
    }
}

#[test]
fn test_deserialize_mismatched_bits_num() {
    let bloom = Bloom::with_hints(100, 0.1, test_hasher());
    let mut serialized = bloom.serialize().expect("serialization failed").to_vec();

    // bits_num is u64 at some position.
    // magic(16) + version(1) + hash_num(4) + hasher_len(4) + hasher_data(136) = 161
    // bits_num is at index 161
    let bits_num_pos = 16 + 1 + 4 + 4 + XXH_SECRET_SIZE_MIN;
    let original_bits_num = u64::from_le_bytes(
        serialized[bits_num_pos..bits_num_pos + 8]
            .try_into()
            .unwrap(),
    );

    // Change bits_num to something that doesn't match the buffer length
    let new_bits_num = original_bits_num + 64;
    serialized[bits_num_pos..bits_num_pos + 8].copy_from_slice(&new_bits_num.to_le_bytes());

    let result: Result<Bloom<XxHasher>, _> = Bloom::deserialize(&serialized);
    match result {
        Err(DeserializeError::BufferTooShort { .. }) => {}
        _ => panic!(
            "Expected BufferTooShort due to mismatched bits_num, got {:?}",
            result
        ),
    }
}

#[test]
fn test_deserialize_unaligned_buffer() {
    let bloom = Bloom::with_hints(100, 0.1, test_hasher());
    let mut serialized = bloom.serialize().expect("serialization failed").to_vec();

    // Add one extra byte
    serialized.push(0);

    let result: Result<Bloom<XxHasher>, _> = Bloom::deserialize(&serialized);
    match result {
        Err(DeserializeError::BufferTooShort { .. }) => {}
        _ => panic!(
            "Expected BufferTooShort due to unaligned buffer, got {:?}",
            result
        ),
    }
}

#[test]
fn test_deserialize_zero_bits_num() {
    let bloom = Bloom::with_hints(100, 0.1, test_hasher());
    let mut serialized = bloom.serialize().expect("serialization failed").to_vec();

    let bits_num_pos = 16 + 1 + 4 + 4 + XXH_SECRET_SIZE_MIN;
    serialized[bits_num_pos..bits_num_pos + 8].copy_from_slice(&0u64.to_le_bytes());

    // We have to truncate the buffer to match expected_rem_size = 0
    serialized.truncate(bits_num_pos + 8);

    let result: Result<Bloom<XxHasher>, _> = Bloom::deserialize(&serialized);
    match result {
        Err(DeserializeError::BufferTooShort { .. }) => {}
        _ => panic!("Expected error for zero bits_num, got {:?}", result),
    }
}
