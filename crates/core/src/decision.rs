//! Culling decisions: pick/reject flags and star ratings.

use serde::{Deserialize, Serialize};

/// Pick or reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    #[default]
    Unflagged,
    Pick,
    Reject,
}

impl Flag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unflagged => "unflagged",
            Self::Pick => "pick",
            Self::Reject => "reject",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [Self::Unflagged, Self::Pick, Self::Reject]
            .into_iter()
            .find(|f| f.as_str() == s)
    }
}

/// Star rating, 0 (none) to 5.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub struct Rating(u8);

impl Rating {
    pub const MAX: u8 = 5;

    pub fn new(stars: u8) -> Option<Self> {
        (stars <= Self::MAX).then_some(Self(stars))
    }

    pub fn stars(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Rating {
    type Error = String;

    fn try_from(stars: u8) -> Result<Self, Self::Error> {
        Self::new(stars).ok_or_else(|| format!("rating must be 0-5, got {stars}"))
    }
}

impl From<Rating> for u8 {
    fn from(r: Rating) -> Self {
        r.0
    }
}

/// A photo's culling decision. Photos without one are unflagged and unrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Decision {
    pub flag: Flag,
    pub rating: Rating,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratings_are_limited_to_five_stars() {
        assert_eq!(Rating::new(5).unwrap().stars(), 5);
        assert!(Rating::new(6).is_none());
        assert!(serde_json::from_str::<Rating>("9").is_err());
        assert_eq!(
            serde_json::from_str::<Rating>("3").unwrap(),
            Rating::new(3).unwrap()
        );
    }

    #[test]
    fn flag_names_round_trip() {
        for f in [Flag::Unflagged, Flag::Pick, Flag::Reject] {
            assert_eq!(Flag::from_name(f.as_str()), Some(f));
        }
        assert_eq!(serde_json::to_string(&Flag::Pick).unwrap(), "\"pick\"");
    }
}
