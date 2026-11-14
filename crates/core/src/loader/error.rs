use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum LoadError {
    DirectoryNotFound(PathBuf),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    RonParse {
        path: PathBuf,
        source: Box<ron::error::SpannedError>,
    },
    Registration(String),
    InvalidCountryTag(String),
    InvalidDate(String),
    UnknownRecipeKey(String),
    UnknownResourceKey(String),
    UnknownPopType(String),
    UnknownStructureKey(String),
    OrphanProvince {
        province_id: u16,
        region_id: u16,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryNotFound(p) => write!(f, "Directory not found: {:?}", p),
            Self::Io { path, source } => write!(f, "IO error at {:?}: {}", path, source),
            Self::RonParse { path, source } => {
                let line = source.span.start.line;
                let col = source.span.start.col;

                write!(
                    f,
                    "Failed to parse file {} at line {}, col {} {}",
                    path.display(),
                    line,
                    col,
                    source.code
                )
            }
            Self::Registration(msg) => write!(f, "Definition registration error: {}", msg),
            Self::InvalidCountryTag(tag) => write!(f, "Invalid CountryTag: '{}'", tag),
            Self::InvalidDate(date) => write!(f, "Invalid date format: '{}'", date),
            Self::UnknownRecipeKey(key) => write!(f, "Unknown recipe_key: '{}'", key),
            Self::UnknownResourceKey(key) => write!(f, "Unknown resource_key: '{}'", key),
            Self::UnknownPopType(key) => write!(f, "Unknown population type: '{}'", key),
            Self::UnknownStructureKey(key) => write!(f, "Unknown structure key: '{}'", key),
            Self::OrphanProvince {
                province_id,
                region_id,
            } => {
                write!(
                    f,
                    "Province {} references non-existent region {}",
                    province_id, region_id
                )
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::RonParse { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
