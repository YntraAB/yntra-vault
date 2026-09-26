//! Shamir's Secret Sharing Scheme (2-of-3 recovery split) in GF(256).

use rand::Rng;
use zeroize::Zeroizing;

/// Addition in GF(256) is equivalent to bitwise XOR.
pub fn gf_add(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Subtraction in GF(256) is equivalent to bitwise XOR.
pub fn gf_sub(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Fixed-round multiplication without secret-indexed lookup tables.
pub fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut result = 0u8;
    for _ in 0..8 {
        result ^= a & 0u8.wrapping_sub(b & 1);
        a = (a << 1) ^ (0x1b & 0u8.wrapping_sub(a >> 7));
        b >>= 1;
    }
    result
}

pub fn gf_div(a: u8, b: u8) -> crate::Result<u8> {
    if b == 0 { return Err(crate::error::VaultError::InvalidFormat("Division by zero".into())); }
    let mut inverse = 1u8;
    for _ in 0..254 { inverse = gf_mul(inverse,b); }
    Ok(gf_mul(a,inverse))
}

/// Split a secret key into 3 shares using a 2-of-3 threshold.
/// The secret can be between 1 and 1024 bytes. Any 2 shares can reconstruct the original secret.
pub fn split_secret(secret: &[u8]) -> crate::Result<Vec<String>> {
    if secret.is_empty() || secret.len() > 1024 {
        return Err(crate::error::VaultError::SerializationError(
            "Secret length must be between 1 and 1024 bytes".into(),
        ));
    }

    let mut share1 = Zeroizing::new(Vec::with_capacity(secret.len()));
    let mut share2 = Zeroizing::new(Vec::with_capacity(secret.len()));
    let mut share3 = Zeroizing::new(Vec::with_capacity(secret.len()));

    for &s in secret {
        // Uniform over the entire field, including zero, for perfect single-share secrecy.
        let a: u8 = rand::rng().random();

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

    // Format as YNTRA-SHARE[X]-[hex]
    Ok(vec![
        format!("YNTRA-SHARE1-{}", data_encoding::HEXLOWER.encode(&share1)),
        format!("YNTRA-SHARE2-{}", data_encoding::HEXLOWER.encode(&share2)),
        format!("YNTRA-SHARE3-{}", data_encoding::HEXLOWER.encode(&share3)),
    ])
}

/// Parse a share string formatted as `YNTRA-SHARE[X]-[hex]` (or legacy `SL-SHARE[X]-[hex]`) and return its coordinate and raw bytes.
pub fn parse_share(share_str: &str) -> crate::Result<(u8, Vec<u8>)> {
    if share_str.len() > 2080 { return Err(crate::error::VaultError::InvalidFormat("Recovery share too long".into())); }
    let trimmed = share_str.trim();
    let upper = Zeroizing::new(trimmed.to_ascii_uppercase());
    if !upper.starts_with("YNTRA-SHARE") && !upper.starts_with("SL-SHARE") {
        return Err(crate::error::VaultError::InvalidFormat(
            "Share must start with YNTRA-SHARE or SL-SHARE".into(),
        ));
    }

    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() != 3 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Invalid share string format".into(),
        ));
    }

    // Parse coordinate
    let coord_str = parts[1].to_ascii_uppercase();
    if coord_str.len() != 6 || !coord_str.starts_with("SHARE") {
        return Err(crate::error::VaultError::InvalidFormat(
            "Invalid share identifier".into(),
        ));
    }
    let coord = coord_str[5..].parse::<u8>().map_err(|_| {
        crate::error::VaultError::InvalidFormat("Invalid share coordinate".into())
    })?;

    if !(1..=3).contains(&coord) {
        return Err(crate::error::VaultError::InvalidFormat(
            "Share coordinate must be between 1 and 3".into(),
        ));
    }

    // Parse hex (case-tolerant)
    let hex_data = Zeroizing::new(parts[2].to_ascii_lowercase());
    let raw_bytes = data_encoding::HEXLOWER
        .decode(hex_data.as_bytes())
        .map_err(|e| crate::error::VaultError::InvalidFormat(format!("Invalid share hex: {}", e)))?;

    if raw_bytes.is_empty() || raw_bytes.len() > 1024 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Share data must be between 1 and 1024 bytes".into(),
        ));
    }

    Ok((coord, raw_bytes))
}

/// Reconstruct the secret using any two parsed shares.
pub fn reconstruct_secret(share_a: &str, share_b: &str) -> crate::Result<Vec<u8>> {
    let (x1, y1_vec) = parse_share(share_a)?;
    let y1_vec = Zeroizing::new(y1_vec);
    let (x2, y2_vec) = parse_share(share_b)?;
    let y2_vec = Zeroizing::new(y2_vec);

    if x1 == x2 {
        return Err(crate::error::VaultError::InvalidFormat(
            "Cannot reconstruct using identical shares".into(),
        ));
    }

    if y1_vec.len() != y2_vec.len() {
        return Err(crate::error::VaultError::InvalidFormat(
            "Share byte lengths do not match".into(),
        ));
    }

    let len = y1_vec.len();
    let mut secret = Vec::with_capacity(len);

    for i in 0..len {
        let y1 = y1_vec[i];
        let y2 = y2_vec[i];

        // Lagrange interpolation at x = 0:
        // s = (x2 * y1 ^ x1 * y2) / (x1 ^ x2)
        let num = gf_add(gf_mul(x2, y1), gf_mul(x1, y2));
        let den = gf_add(x1, x2);
        let s = gf_div(num, den)?;

        secret.push(s);
    }

    Ok(secret)
}

/// Split a plaintext master password directly into 3 recovery shares using a 2-of-3 threshold.
pub fn split_password(password: &str) -> crate::Result<Vec<String>> {
    if password.is_empty() {
        return Err(crate::error::VaultError::SerializationError(
            "Password cannot be empty".into(),
        ));
    }
    split_secret(password.as_bytes())
}

/// Reconstruct the original UTF-8 master password from any two recovery shares.
pub fn reconstruct_password(share_a: &str, share_b: &str) -> crate::Result<String> {
    let bytes = reconstruct_secret(share_a, share_b)?;
    String::from_utf8(bytes).map_err(|e| {
        crate::error::VaultError::InvalidFormat(format!("Reconstructed secret is not valid UTF-8: {}", e))
    })
}

/// Reconstruct the secret from shares and return it as hex.
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
        assert_eq!(gf_div(0, 5).unwrap(), 0);
        assert_eq!(gf_div(10, 1).unwrap(), 10);
        assert_eq!(gf_div(5, 5).unwrap(), 1);
        assert!(gf_div(5, 0).is_err());

        // Multiplication & Division compatibility
        let a = 42;
        let b = 137;
        let c = gf_mul(a, b);
        assert_eq!(gf_div(c, b).unwrap(), a);
        assert_eq!(gf_div(c, a).unwrap(), b);
    }

    #[test]
    fn test_split_and_reconstruct_roundtrip() {
        let secret = [7u8; 32];
        let shares = split_secret(&secret).unwrap();
        assert_eq!(shares.len(), 3);
        assert!(shares[0].starts_with("YNTRA-SHARE1-"));
        assert!(shares[1].starts_with("YNTRA-SHARE2-"));
        assert!(shares[2].starts_with("YNTRA-SHARE3-"));

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
    fn test_legacy_sl_share_backward_compatibility() {
        let secret = [42u8; 32];
        let yntra_shares = split_secret(&secret).unwrap();

        // Convert shares to legacy SL-SHARE format
        let legacy_share1 = yntra_shares[0].replace("YNTRA-SHARE", "SL-SHARE");
        let legacy_share2 = yntra_shares[1].replace("YNTRA-SHARE", "SL-SHARE");
        let legacy_share3 = yntra_shares[2].replace("YNTRA-SHARE", "SL-SHARE");

        // Test reconstruct with two legacy shares
        let rec_legacy = reconstruct_secret(&legacy_share1, &legacy_share2).unwrap();
        assert_eq!(rec_legacy, secret);

        // Test reconstruct with mixed modern and legacy shares
        let rec_mixed = reconstruct_secret(&yntra_shares[0], &legacy_share3).unwrap();
        assert_eq!(rec_mixed, secret);
    }

    #[test]
    fn test_case_tolerant_share_parsing() {
        let secret = b"super-secret-key-case-test";
        let shares = split_secret(secret).unwrap();

        // 1. Lowercase prefix: yntra-share1-...
        let lower_share = shares[0].to_ascii_lowercase();
        // 2. Uppercase hex: YNTRA-SHARE2-ABCDEF...
        let parts: Vec<&str> = shares[1].split('-').collect();
        let upper_hex_share = format!("{}-{}-{}", parts[0], parts[1], parts[2].to_ascii_uppercase());

        let rec = reconstruct_secret(&lower_share, &upper_hex_share).unwrap();
        assert_eq!(rec, secret);
    }

    #[test]
    fn test_invalid_share_formatting() {
        assert!(parse_share("YNTRA-SHARE1-invalidhex").is_err());
        assert!(parse_share("SL-SHARE1-invalidhex").is_err());
        assert!(parse_share("INVALID-1-abc").is_err());
        assert!(parse_share("YNTRA-SHARE4-0000000000000000000000000000000000000000000000000000000000000000").is_err());
        assert!(parse_share("SL-SHARE4-0000000000000000000000000000000000000000000000000000000000000000").is_err());
    }

    #[test]
    fn test_variable_length_secrets() {
        let lengths = [1, 5, 16, 32, 64, 128, 512, 1024];
        for len in lengths {
            let secret = vec![0x42u8; len];
            let shares = split_secret(&secret).unwrap();
            let rec = reconstruct_secret(&shares[0], &shares[2]).unwrap();
            assert_eq!(rec, secret);
        }

        assert!(split_secret(&[]).is_err());
        assert!(split_secret(&vec![0u8; 1025]).is_err());
    }

    #[test]
    fn test_split_and_reconstruct_password_roundtrip() {
        let passwords = [
            "correct-horse-battery-staple",
            "P@ssw0rd!#$123",
            "A",
            "LongPasswordWithSpecialCharacters_!@#$%^&*()_+{}[]:;\"'<>?,./~`1234567890",
            "UnicodePassword_🔑_🔒_åäö_Éxàmple",
        ];

        for pass in passwords {
            let shares = split_password(pass).unwrap();
            assert_eq!(shares.len(), 3);

            let rec12 = reconstruct_password(&shares[0], &shares[1]).unwrap();
            assert_eq!(rec12, pass);

            let rec23 = reconstruct_password(&shares[1], &shares[2]).unwrap();
            assert_eq!(rec23, pass);

            let rec13 = reconstruct_password(&shares[0], &shares[2]).unwrap();
            assert_eq!(rec13, pass);
        }
    }
}
