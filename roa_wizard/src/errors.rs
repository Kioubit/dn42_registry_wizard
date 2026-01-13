use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug)]
pub struct GenerationError {
    pub kind: GenerationErrorKind,
    pub filename: Option<String>,
    pub line_number: Option<usize>,
}

impl GenerationError {
    pub(crate) fn new(kind: GenerationErrorKind, filename: Option<impl Into<String>>, line_number: Option<usize>) -> Self {
        let filename = filename.map(Into::into);
        GenerationError { kind, filename, line_number }
    }
}

impl std::error::Error for GenerationError {}

#[derive(Debug)]
pub enum GenerationErrorKind {
    ParseError(ParseError),
    FilterSetError(FilterSetError),
    IoError(io::Error),
    CancelledDueToWarning()
}
#[derive(Debug)]
pub enum ParseError {
    MissingField(&'static str),
    BadOrigin(String),
    BadPrefix{prefix_string: String, error: String},
    BadFileName(String),
    TypeMismatch { expected_v6: bool },
    InvalidMaxLen(String),
}
#[derive(Debug)]
pub enum FilterSetError {
    InvalidRow(),
    InvalidField(&'static str),
}

impl From<ParseError> for GenerationErrorKind {
    fn from(err: ParseError) -> GenerationErrorKind {
        GenerationErrorKind::ParseError(err)
    }
}

impl From<FilterSetError> for GenerationErrorKind {
    fn from(err: FilterSetError) -> GenerationErrorKind {
        GenerationErrorKind::FilterSetError(err)
    }
}

impl Display for GenerationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // 1. Handle the location context
        if let Some(ref file) = self.filename {
            write!(f, "in {file}")?;
            if let Some(line) = self.line_number {
                write!(f, ":{line}")?;
            }
            write!(f, ": ")?;
        } else if let Some(line) = self.line_number {
            write!(f, "line {line}: ")?;
        }

        // 2. Delegate the actual error message to the kind
        write!(f, "{}", self.kind)
    }
}

impl Display for GenerationErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError(e) => write!(f, "parse error: {e}"),
            Self::FilterSetError(e) => write!(f, "filter set error: {e}"),
            Self::IoError(e) => write!(f, "IO error: {e}"),
            Self::CancelledDueToWarning() => write!(f, "generation cancelled due to warnings"),
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(s) => write!(f, "missing field: '{s}'"),
            Self::BadOrigin(s) => write!(f, "invalid origin: '{s}'"),
            Self::BadPrefix { prefix_string, error } => {
                write!(f, "invalid prefix '{prefix_string}': {error}")
            }
            Self::BadFileName(s) => write!(f, "invalid filename: '{s}'"),
            Self::TypeMismatch { expected_v6 } => {
                let ver = if *expected_v6 { "v6" } else { "v4" };
                write!(f, "type mismatch: expected IP{ver}")
            }
            Self::InvalidMaxLen(s) => write!(f, "invalid max length: {s}"),
        }
    }
}

impl Display for FilterSetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRow() => write!(f, "invalid row"),
            Self::InvalidField(s) => write!(f, "invalid field: '{s}'"),
        }
    }
}
