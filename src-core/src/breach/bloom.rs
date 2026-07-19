//! Offline Bloom filter password breach pre-checker.

use sha2::{Sha256, Digest};

const BLOOM_DATA: &[u8; 65536] = include_bytes!("bloom.bin");
const TOTAL_BITS: u64 = 524288;

/// Evaluates if a password might be breached by testing the local 64KB Bloom filter.
/// Returns true if a breach is suspected, and false if it is guaranteed safe.
pub fn is_breach_suspected(password: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();

    // Split 32-byte hash into four 8-byte integers
    let val1 = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    let val2 = u64::from_be_bytes(hash[8..16].try_into().unwrap());
    let val3 = u64::from_be_bytes(hash[16..24].try_into().unwrap());
    let val4 = u64::from_be_bytes(hash[24..32].try_into().unwrap());

    // Calculate bit indices
    let idx1 = val1 % TOTAL_BITS;
    let idx2 = val2 % TOTAL_BITS;
    let idx3 = val3 % TOTAL_BITS;
    let idx4 = val4 % TOTAL_BITS;

    // Check if all 4 bits are set in BLOOM_DATA
    is_bit_set(idx1) && is_bit_set(idx2) && is_bit_set(idx3) && is_bit_set(idx4)
}

fn is_bit_set(idx: u64) -> bool {
    let byte_idx = (idx / 8) as usize;
    let bit_pos = (idx % 8) as u8;
    (BLOOM_DATA[byte_idx] & (1 << bit_pos)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_bloom_filter_basic() {
        // Since bloom.bin was initialized to all zeros, this should return false for everything
        assert!(!is_breach_suspected("highly_secure_non_existent_password_12345!"));
    }

    #[test]
    fn test_generate_and_populate_bloom_filter() {
        // Populate the bloom filter with common passwords
        let common_passwords = vec![
            "password", "123456", "12345678", "123456789", "12345",
            "qwerty", "password123", "admin", "admin123", "root",
        ];

        let mut data = vec![0u8; 65536];

        for pw in common_passwords {
            let mut hasher = Sha256::new();
            hasher.update(pw.as_bytes());
            let hash = hasher.finalize();

            let val1 = u64::from_be_bytes(hash[0..8].try_into().unwrap());
            let val2 = u64::from_be_bytes(hash[8..16].try_into().unwrap());
            let val3 = u64::from_be_bytes(hash[16..24].try_into().unwrap());
            let val4 = u64::from_be_bytes(hash[24..32].try_into().unwrap());

            let idx1 = val1 % TOTAL_BITS;
            let idx2 = val2 % TOTAL_BITS;
            let idx3 = val3 % TOTAL_BITS;
            let idx4 = val4 % TOTAL_BITS;

            for &idx in &[idx1, idx2, idx3, idx4] {
                let byte_idx = (idx / 8) as usize;
                let bit_pos = (idx % 8) as u8;
                data[byte_idx] |= 1 << bit_pos;
            }
        }

        fs::write("src/breach/bloom.bin", &data).unwrap();
    }
}
