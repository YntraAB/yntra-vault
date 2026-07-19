//! Shamir's Secret Sharing Scheme (2-of-3 recovery split) in GF(256).

use std::sync::OnceLock;
use rand::Rng;

static TABLES: OnceLock<([u8; 256], [u8; 256])> = OnceLock::new();

/// Retrieve or lazily initialize the GF(256) logarithm and exponentiation tables.
/// Uses the generator g = 3 and the AES primitive polynomial x^8 + x^4 + x^3 + x + 1 (0x11b).
fn get_tables() -> &'static ([u8; 256], [u8; 256]) {
    TABLES.get_or_init(|| {
        let mut exp = [0u8; 256];
        let mut log = [0u8; 256];
        let mut val = 1u8;
        for i in 0..255 {
            exp[i] = val;
            log[val as usize] = i as u8;

            // Multiply by 3 in GF(256): (val << 1) ^ val
            let mut next = (val << 1) ^ val;
            if (val & 0x80) != 0 {
                next ^= 0x1b; // Reduce by primitive polynomial 0x11b
            }
            val = next;
        }
        exp[255] = exp[0];
        (exp, log)
    })
}

/// Addition in GF(256) is equivalent to bitwise XOR.
pub fn gf_add(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Subtraction in GF(256) is equivalent to bitwise XOR.
pub fn gf_sub(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Multiplication in GF(256) using log/exp tables.
pub fn gf_mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let (exp, log) = get_tables();
    let log_sum = (log[a as usize] as u16) + (log[b as usize] as u16);
    exp[(log_sum % 255) as usize]
}

/// Division in GF(256) using log/exp tables.
pub fn gf_div(a: u8, b: u8) -> u8 {
    assert!(b != 0, "Division by zero in GF(256)");
    if a == 0 {
        return 0;
    }
    let (exp, log) = get_tables();
    let log_diff = (log[a as usize] as i16) - (log[b as usize] as i16);
    let index = if log_diff < 0 { log_diff + 255 } else { log_diff };
    exp[index as usize]
}

/// Split a secret key (expected to be 32 bytes) into 3 shares using a 2-of-3 threshold.
/// Any 2 of these shares can reconstruct the original secret.
pub fn split_secret(secret: &[u8]) -> crate::Result<Vec<String>> {
    if secret.len() != 32 {
        return Err(crate::error::VaultError::SerializationError(
            "Secret must be exactly 32 bytes".into(),
        ));
    }

    let mut share1 = Vec::with_capacity(32);
    let mut share2 = Vec::with_capacity(32);
    let mut share3 = Vec::with_capacity(32);

    for &s in secret {
        // Generate random coefficient `a` (cannot be 0 to avoid identical shares)
        let mut a = 0u8;
        while a == 0 {
            a = rand::rng().random();
        }

        // f(x) = (a * x) ^ s
        // For x = 1: f(1) = a ^ s
        // For x = 2: f(2) = (a * 2) ^ s
        // For x = 3: f(3) = (a * 3) ^ s
        let y1 = gf_add(gf_mul(a, 1), s);
        let y2 = gf_add(gf_mul(a, 2), s);
        let y3 = gf_add(gf_mul(a, 3), s);

        share1.push(y1);
        share2.push(y2);
        share3.push(y3);
    }

    // Format as SL-SHARE[X]-[hex]
    Ok(vec![
        format!("SL-SHARE1-{}", data_encoding::HEXLOWER.encode(&share1)),
        format!("SL-SHARE2-{}", data_encoding::HEXLOWER.encode(&share2)),
        format!("SL-SHARE3-{}", data_encoding::HEXLOWER.encode(&share3)),
    ])
}

/// Parse a share string formatted as `SL-SHARE[X]-[hex]` and return its coordinate and raw bytes.
pub fn parse_share(share_str: &str) -> crate::Result<(u8, Vec<u8>)> {
    let trimmed = share_str.trim();
    if !trimmed.starts_with("SL-SHARE") {
        return Err(crate::error::VaultError::InvalidFormat(
            "Share must start with SL-SHARE".into(),
        ));
    }

    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() != 3 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Invalid share string format".into(),
        ));
    }

    // Parse coordinate
    let coord_str = parts[1];
    if coord_str.len() != 6 || !coord_str.starts_with("SHARE") {
        return Err(crate::error::VaultError::InvalidFormat(
            "Invalid share identifier".into(),
        ));
    }
    let coord = coord_str[5..].parse::<u8>().map_err(|_| {
        crate::error::VaultError::InvalidFormat("Invalid share coordinate".into())
    })?;

    if coord < 1 || coord > 3 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Share coordinate must be between 1 and 3".into(),
        ));
    }

    // Parse hex
    let hex_data = parts[2];
    let raw_bytes = data_encoding::HEXLOWER
        .decode(hex_data.as_bytes())
        .map_err(|e| crate::error::VaultError::InvalidFormat(format!("Invalid share hex: {}", e)))?;

    if raw_bytes.len() != 32 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Share data must be exactly 32 bytes".into(),
        ));
    }

    Ok((coord, raw_bytes))
}

/// Reconstruct the 32-byte secret key using any two parsed shares.
pub fn reconstruct_secret(share_a: &str, share_b: &str) -> crate::Result<Vec<u8>> {
    let (x1, y1_vec) = parse_share(share_a)?;
    let (x2, y2_vec) = parse_share(share_b)?;

    if x1 == x2 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Cannot reconstruct using identical shares".into(),
        ));
    }

    let mut secret = Vec::with_capacity(32);

    for i in 0..32 {
        let y1 = y1_vec[i];
        let y2 = y2_vec[i];

        // Lagrange interpolation at x = 0:
        // s = (x2 * y1 ^ x1 * y2) / (x1 ^ x2)
        let num = gf_add(gf_mul(x2, y1), gf_mul(x1, y2));
        let den = gf_add(x1, x2);
        let s = gf_div(num, den);

        secret.push(s);
    }

    Ok(secret)
}

/// Helper function to split a string password by hashing it to 32 bytes first.
pub fn split_password(password: &str) -> crate::Result<Vec<String>> {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();
    split_secret(&hash)
}

/// Helper function to reconstruct a password hash and return it as a hex string.
pub fn reconstruct_password_to_hex(share_a: &str, share_b: &str) -> crate::Result<String> {
    let secret = reconstruct_secret(share_a, share_b)?;
    Ok(data_encoding::HEXLOWER.encode(&secret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gf_arithmetic() {
        // XOR arithmetic
        assert_eq!(gf_add(5, 3), 6);
        assert_eq!(gf_sub(10, 10), 0);

        // Multiplication identities
        assert_eq!(gf_mul(0, 5), 0);
        assert_eq!(gf_mul(5, 0), 0);
        assert_eq!(gf_mul(5, 1), 5);
        assert_eq!(gf_mul(1, 10), 10);

        // Division identities
        assert_eq!(gf_div(0, 5), 0);
        assert_eq!(gf_div(10, 1), 10);
        assert_eq!(gf_div(5, 5), 1);

        // Multiplication & Division compatibility
        let a = 42;
        let b = 137;
        let c = gf_mul(a, b);
        assert_eq!(gf_div(c, b), a);
        assert_eq!(gf_div(c, a), b);
    }

    #[test]
    fn test_split_and_reconstruct_roundtrip() {
        let secret = [7u8; 32];
        let shares = split_secret(&secret).unwrap();
        assert_eq!(shares.len(), 3);

        // Try reconstruct 1 & 2
        let rec12 = reconstruct_secret(&shares[0], &shares[1]).unwrap();
        assert_eq!(rec12, secret);

        // Try reconstruct 2 & 3
        let rec23 = reconstruct_secret(&shares[1], &shares[2]).unwrap();
        assert_eq!(rec23, secret);

        // Try reconstruct 1 & 3
        let rec13 = reconstruct_secret(&shares[0], &shares[2]).unwrap();
        assert_eq!(rec13, secret);
    }

    #[test]
    fn test_invalid_share_formatting() {
        assert!(parse_share("SL-SHARE1-invalidhex").is_err());
        assert!(parse_share("INVALID-1-abc").is_err());
        assert!(parse_share("SL-SHARE4-0000000000000000000000000000000000000000000000000000000000000000").is_err());
    }
}
