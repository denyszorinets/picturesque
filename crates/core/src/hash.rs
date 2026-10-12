//! Content-hash identity of files.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// BLAKE3 digest of a file's bytes. Two files with the same hash are
/// treated as the same file (ADR-0008). Computing hashes is an
/// infrastructure concern; this type only carries the value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hexadecimal, 64 characters.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Error parsing a [`ContentHash`] from hexadecimal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseContentHashError;

impl fmt::Display for ParseContentHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("content hash must be 64 hexadecimal characters")
    }
}

impl std::error::Error for ParseContentHashError {}

impl FromStr for ContentHash {
    type Err = ParseContentHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 || !s.is_ascii() {
            return Err(ParseContentHashError);
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte =
                u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| ParseContentHashError)?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self.to_hex())
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn sample() -> ContentHash {
        let mut bytes = [0u8; 32];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = i as u8;
        }
        ContentHash::from_bytes(bytes)
    }

    #[test]
    fn formats_as_lowercase_hex() {
        assert_eq!(sample().to_hex(), HEX);
        assert_eq!(sample().to_string(), HEX);
    }

    #[test]
    fn parses_hex_in_either_case() {
        assert_eq!(HEX.parse::<ContentHash>(), Ok(sample()));
        assert_eq!(HEX.to_uppercase().parse::<ContentHash>(), Ok(sample()));
    }

    #[test]
    fn rejects_wrong_length_or_non_hex() {
        assert_eq!("abc".parse::<ContentHash>(), Err(ParseContentHashError));
        assert_eq!(
            "zz".repeat(32).parse::<ContentHash>(),
            Err(ParseContentHashError)
        );
        assert_eq!(
            "é".repeat(32).parse::<ContentHash>(),
            Err(ParseContentHashError)
        );
    }
}
