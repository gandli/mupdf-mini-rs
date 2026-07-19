use mupdf::Error as MupdfError;

/// Convenience result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors surfaced by the viewer library.
#[derive(Debug)]
pub enum Error {
    /// An error originating from the underlying MuPDF binding.
    Mupdf(MupdfError),
    /// I/O error (file open, save, etc.).
    Io(std::io::Error),
    /// The document contains no pages.
    EmptyDocument,
    /// A requested page index is outside `0..page_count`.
    PageOutOfRange { index: usize, count: usize },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Mupdf(e) => write!(f, "MuPDF error: {e}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::EmptyDocument => write!(f, "document contains no pages"),
            Error::PageOutOfRange { index, count } => {
                write!(f, "page {index} out of range (document has {count} pages)")
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<MupdfError> for Error {
    fn from(e: MupdfError) -> Self {
        Error::Mupdf(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
