//! Prepočet PDF na tlač brožúry alebo zošitov.
//!
//! ```no_run
//! use booklet_core::{impose_file, Mode, Options};
//!
//! let mut opts = Options::default();
//! opts.plan.mode = Mode::Signatures { sheets: 4 };
//! impose_file("kniha.pdf".as_ref(), "kniha-tlac.pdf".as_ref(), &opts)?;
//! # Ok::<_, booklet_core::Error>(())
//! ```

pub mod geom;
pub mod i18n;
pub mod impose;
pub mod plan;

pub use geom::{mm, to_mm, Matrix, Orientation, Paper, Rect};
pub use i18n::Lang;
pub use impose::{impose, impose_file, info, open, FoldMark, Marks, Options, PdfInfo, Summary};
pub use plan::{
    parse_range, Binding, Face, Flip, Mode, Plan, PlanOptions, RangeError, SheetOrder, Side, Slot,
};

/// Chyby knižnice. Hlásenia sú po anglicky; preložené znenie pre
/// používateľa vráti [`i18n::Lang::error`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read the PDF: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("the PDF is password protected")]
    Encrypted,
    #[error("the document contains no pages")]
    NoPages,
    #[error("invalid page range: {0:?}")]
    Range(RangeError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
