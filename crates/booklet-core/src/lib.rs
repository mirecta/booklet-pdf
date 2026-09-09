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
pub mod impose;
pub mod plan;

pub use geom::{mm, to_mm, Matrix, Orientation, Paper, Rect};
pub use impose::{impose, impose_file, info, open, FoldMark, Marks, Options, PdfInfo, Summary};
pub use plan::{parse_range, Binding, Face, Flip, Mode, Plan, PlanOptions, SheetOrder, Side, Slot};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("PDF sa nepodarilo prečítať: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("PDF je chránené heslom")]
    Encrypted,
    #[error("dokument neobsahuje žiadne strany")]
    NoPages,
    #[error("chybný rozsah strán: {0}")]
    Range(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
