//! Command line front-end for booklet-pdf.

use std::path::PathBuf;

use anyhow::{Context, Result};
use booklet_core::{
    impose_file, info, plan, Binding, Flip, FoldMark, Lang, Marks, Mode, Options, Orientation,
    Paper, PlanOptions, SheetOrder,
};
use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(
    name = "booklet",
    about = "Impose a PDF for printing a booklet or signatures (2 pages per sheet)",
    version
)]
struct Cli {
    /// Input PDF.
    input: PathBuf,

    /// Output PDF. Defaults to `<input>-booklet.pdf`.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Folding mode.
    #[arg(short, long, value_enum, default_value_t = ModeArg::Booklet)]
    mode: ModeArg,

    /// Sheets per signature (only for `--mode signatures`).
    #[arg(short = 'n', long, default_value_t = 4)]
    sheets: usize,

    /// Size of the output sheet.
    #[arg(short, long, value_enum, default_value_t = PaperArg::A4)]
    paper: PaperArg,

    /// Orientation of the output sheet.
    #[arg(long, value_enum, default_value_t = OrientationArg::Landscape)]
    orientation: OrientationArg,

    /// Binding side.
    #[arg(short, long, value_enum, default_value_t = BindingArg::Left)]
    binding: BindingArg,

    /// How the printer flips the paper in duplex.
    #[arg(long, value_enum, default_value_t = FlipArg::Short)]
    flip: FlipArg,

    /// Order of the pages in the output.
    #[arg(long, value_enum, default_value_t = OrderArg::Interleaved)]
    order: OrderArg,

    /// Margin around the sheet, in mm.
    #[arg(long, default_value_t = 0.0)]
    margin: f64,

    /// Extra space at the fold, in mm.
    #[arg(long, default_value_t = 0.0)]
    gutter: f64,

    /// Creep compensation, in mm per sheet.
    #[arg(long, default_value_t = 0.0)]
    creep: f64,

    /// Do not scale the pages, keep scale 1:1.
    #[arg(long)]
    no_scale: bool,

    /// How to mark the fold.
    #[arg(long, value_enum, default_value_t = FoldArg::Ticks)]
    fold: FoldArg,

    /// Add crop marks at the sheet edges.
    #[arg(long)]
    crop: bool,

    /// Page range, e.g. `1-8,11`.
    #[arg(long, default_value = "")]
    pages: String,

    /// Password for an encrypted PDF.
    #[arg(long)]
    password: Option<String>,

    /// Language of the output messages. Detected from the locale by default.
    #[arg(long, value_enum)]
    lang: Option<LangArg>,

    /// Only print the plan, write nothing.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ModeArg {
    /// One saddle-stitched booklet.
    Booklet,
    /// Signatures of N sheets each.
    Signatures,
    /// 2 pages per sheet, no folding.
    Twoup,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum PaperArg {
    A2,
    A3,
    A4,
    A5,
    A6,
    Letter,
    Legal,
    Tabloid,
    /// Exactly twice the first source page.
    Source,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OrientationArg {
    Landscape,
    Portrait,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum BindingArg {
    Left,
    Right,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum FlipArg {
    /// Flip on the short edge (usual for a landscape sheet).
    Short,
    /// Flip on the long edge.
    Long,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OrderArg {
    Interleaved,
    FrontsThenBacks,
    FrontsThenBacksReversed,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum LangArg {
    En,
    Sk,
    Cs,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum FoldArg {
    /// Nothing, the sheet stays clean.
    None,
    /// Short ticks at the sheet edges (not drawn over the content).
    Ticks,
    /// Dashed line across the whole sheet.
    Line,
}

fn main() -> Result<()> {
    match run() {
        // `booklet ... | head` closes the pipe; that is not a failure.
        Err(e) if is_broken_pipe(&e) => Ok(()),
        other => other,
    }
}

fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::BrokenPipe)
    })
}

fn run() -> Result<()> {
    use std::io::Write;

    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    let lang = match cli.lang {
        Some(LangArg::En) => Lang::En,
        Some(LangArg::Sk) => Lang::Sk,
        Some(LangArg::Cs) => Lang::Cs,
        None => Lang::detect(),
    };
    let output = cli.output.clone().unwrap_or_else(|| default_output(&cli.input));

    let opts = Options {
        plan: PlanOptions {
            mode: match cli.mode {
                ModeArg::Booklet => Mode::Booklet,
                ModeArg::Signatures => Mode::Signatures { sheets: cli.sheets.max(1) },
                ModeArg::Twoup => Mode::TwoUp,
            },
            binding: match cli.binding {
                BindingArg::Left => Binding::Left,
                BindingArg::Right => Binding::Right,
            },
            flip: match cli.flip {
                FlipArg::Short => Flip::ShortEdge,
                FlipArg::Long => Flip::LongEdge,
            },
            order: match cli.order {
                OrderArg::Interleaved => SheetOrder::Interleaved,
                OrderArg::FrontsThenBacks => SheetOrder::FrontsThenBacks,
                OrderArg::FrontsThenBacksReversed => SheetOrder::FrontsThenBacksReversed,
            },
        },
        paper: match cli.paper {
            PaperArg::A2 => Paper::A2,
            PaperArg::A3 => Paper::A3,
            PaperArg::A4 => Paper::A4,
            PaperArg::A5 => Paper::A5,
            PaperArg::A6 => Paper::A6,
            PaperArg::Letter => Paper::Letter,
            PaperArg::Legal => Paper::Legal,
            PaperArg::Tabloid => Paper::Tabloid,
            PaperArg::Source => Paper::FromSource,
        },
        orientation: match cli.orientation {
            OrientationArg::Landscape => Orientation::Landscape,
            OrientationArg::Portrait => Orientation::Portrait,
        },
        margin_mm: cli.margin,
        gutter_mm: cli.gutter,
        creep_mm: cli.creep,
        scale: !cli.no_scale,
        marks: Marks {
            fold: match cli.fold {
                FoldArg::None => FoldMark::None,
                FoldArg::Ticks => FoldMark::Ticks,
                FoldArg::Line => FoldMark::Line,
            },
            crop: cli.crop,
        },
        range: cli.pages.clone(),
        password: cli.password.clone(),
    };

    if cli.dry_run {
        let info = info(&cli.input, opts.password.as_deref())
            .map_err(|e| anyhow::Error::msg(lang.error(&e)))
            .with_context(|| format!("{}", cli.input.display()))?;
        let selection = plan::parse_range(&opts.range, info.pages)
            .map_err(|e| anyhow::Error::msg(lang.range_error(&e)))?;
        let plan = plan::plan(&selection, &opts.plan);
        write_plan(&mut out, &plan, lang)?;
        return Ok(());
    }

    let summary = impose_file(&cli.input, &output, &opts)
        .map_err(|e| anyhow::Error::msg(lang.error(&e)))
        .with_context(|| format!("{}", cli.input.display()))?;
    let (w, h) = summary.sheet_pt;
    writeln!(
        out,
        "{} → {}\n{}\n{}",
        cli.input.display(),
        output.display(),
        summary_line(&summary.plan, lang),
        lang.sheet_size(booklet_core::to_mm(w), booklet_core::to_mm(h)),
    )?;
    Ok(())
}

/// One-line summary including the signature breakdown.
fn summary_line(plan: &booklet_core::Plan, lang: Lang) -> String {
    let mut line = lang.counts(plan.source_pages, plan.sheets, plan.output_pages(), plan.blanks);
    if plan.is_grouped() {
        line.push('\n');
        line.push_str(&lang.signatures_summary(plan.signatures.len(), &plan.signature_breakdown()));
    }
    line
}

fn default_output(input: &std::path::Path) -> PathBuf {
    let stem = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    input.with_file_name(format!("{stem}-booklet.pdf"))
}

fn write_plan(
    out: &mut impl std::io::Write,
    plan: &booklet_core::Plan,
    lang: Lang,
) -> std::io::Result<()> {
    writeln!(out, "{}", summary_line(plan, lang))?;
    for (i, side) in plan.sides.iter().enumerate() {
        let cell = |s: &booklet_core::Slot| match s.page {
            Some(p) => {
                let n = p + 1;
                if s.rotate180 {
                    format!("{n}↻")
                } else {
                    n.to_string()
                }
            }
            None => "—".to_string(),
        };
        writeln!(
            out,
            "{:>3}. {} {:>2} {:<5} · {} {:<2} │ {:>6} │ {:<6}",
            i + 1,
            lang.word_sheet(1),
            side.sheet + 1,
            lang.face_label(side.face),
            lang.word_signature(1),
            side.signature + 1,
            cell(&side.slots[0]),
            cell(&side.slots[1]),
        )?;
    }
    Ok(())
}
