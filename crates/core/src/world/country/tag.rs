use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash, Serialize, Deserialize)]
pub struct CountryTag([u8; 3]);

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidLengthError;

impl TryFrom<&str> for CountryTag {
    type Error = InvalidLengthError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let bytes = value.as_bytes();
        let array: [u8; 3] = bytes.try_into().map_err(|_| InvalidLengthError)?;
        Ok(Self(array))
    }
}

impl CountryTag {
    pub fn new(tag: &str) -> CountryTag {
        Self(tag.as_bytes().try_into().unwrap())
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap()
    }
}

impl fmt::Display for CountryTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
