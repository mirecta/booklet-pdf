//! Príkazová riadka pre booklet-pdf.

use std::path::PathBuf;

use anyhow::{Context, Result};
use booklet_core::{
    impose_file, info, plan, Binding, Face, Flip, FoldMark, Marks, Mode, Options, Orientation,
    Paper, PlanOptions, SheetOrder,
};
use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(
    name = "booklet",
    about = "Prepočíta PDF na tlač brožúry alebo zošitov (2 strany na list)",
    version
)]
struct Cli {
    /// Vstupné PDF.
    input: PathBuf,

    /// Výstupné PDF. Predvolene `<vstup>-booklet.pdf`.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Režim skladania.
    #[arg(short, long, value_enum, default_value_t = ModeArg::Booklet)]
    mode: ModeArg,

    /// Počet listov v jednom zošite (len pre `--mode signatures`).
    #[arg(short = 'n', long, default_value_t = 4)]
    sheets: usize,

    /// Formát výstupného listu.
    #[arg(short, long, value_enum, default_value_t = PaperArg::A4)]
    paper: PaperArg,

    /// Orientácia výstupného listu.
    #[arg(long, value_enum, default_value_t = OrientationArg::Landscape)]
    orientation: OrientationArg,

    /// Strana väzby.
    #[arg(short, long, value_enum, default_value_t = BindingArg::Left)]
    binding: BindingArg,

    /// Ako tlačiareň obracia papier pri duplexe.
    #[arg(long, value_enum, default_value_t = FlipArg::Short)]
    flip: FlipArg,

    /// Poradie strán vo výstupe.
    #[arg(long, value_enum, default_value_t = OrderArg::Interleaved)]
    order: OrderArg,

    /// Okraj listu v mm.
    #[arg(long, default_value_t = 0.0)]
    margin: f64,

    /// Medzera v mieste prehybu v mm.
    #[arg(long, default_value_t = 0.0)]
    gutter: f64,

    /// Kompenzácia skladania (creep) v mm na list.
    #[arg(long, default_value_t = 0.0)]
    creep: f64,

    /// Nezmenšovať strany, ponechať mierku 1:1.
    #[arg(long)]
    no_scale: bool,

    /// Ako vyznačiť miesto prehybu.
    #[arg(long, value_enum, default_value_t = FoldArg::Ticks)]
    fold: FoldArg,

    /// Pridať orezové značky na hrany listu.
    #[arg(long)]
    crop: bool,

    /// Rozsah strán, napr. `1-8,11`.
    #[arg(long, default_value = "")]
    pages: String,

    /// Heslo k zašifrovanému PDF.
    #[arg(long)]
    password: Option<String>,

    /// Len vypíš plán, nič nezapisuj.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ModeArg {
    /// Jedna brožúra zošitá v strede.
    Booklet,
    /// Zošity po N listoch.
    Signatures,
    /// 2 strany na list bez skladania.
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
    /// Presne dvojnásobok prvej strany zdroja.
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
    /// Obrat okolo krátkej hrany (bežné pre list na ležato).
    Short,
    /// Obrat okolo dlhej hrany.
    Long,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OrderArg {
    Interleaved,
    FrontsThenBacks,
    FrontsThenBacksReversed,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum FoldArg {
    /// Nič, list zostane čistý.
    None,
    /// Krátke značky pri hranách listu (nekreslia sa cez obsah).
    Ticks,
    /// Prerušovaná čiara cez celý list.
    Line,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
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
            .with_context(|| format!("nedá sa načítať {}", cli.input.display()))?;
        let selection = plan::parse_range(&opts.range, info.pages).map_err(anyhow::Error::msg)?;
        let plan = plan::plan(&selection, &opts.plan);
        print_plan(&plan);
        return Ok(());
    }

    let summary = impose_file(&cli.input, &output, &opts)
        .with_context(|| format!("nedá sa spracovať {}", cli.input.display()))?;
    let (w, h) = summary.sheet_pt;
    println!(
        "{} → {}\n{} zdrojových strán, {} listov, {} strán výstupu, {} prázdnych miest\nlist {:.0}×{:.0} mm",
        cli.input.display(),
        output.display(),
        summary.plan.source_pages,
        summary.plan.sheets,
        summary.plan.output_pages(),
        summary.plan.blanks,
        booklet_core::to_mm(w),
        booklet_core::to_mm(h),
    );
    Ok(())
}

fn default_output(input: &std::path::Path) -> PathBuf {
    let stem = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    input.with_file_name(format!("{stem}-booklet.pdf"))
}

fn print_plan(plan: &booklet_core::Plan) {
    println!(
        "{} zdrojových strán, {} listov, {} strán výstupu, {} prázdnych miest",
        plan.source_pages,
        plan.sheets,
        plan.output_pages(),
        plan.blanks
    );
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
        println!(
            "{:>3}. list {:>2} {:<4} zošit {:<2} │ {:>6} │ {:<6}",
            i + 1,
            side.sheet + 1,
            match side.face {
                Face::Front => "líce",
                Face::Back => "rub",
            },
            side.signature + 1,
            cell(&side.slots[0]),
            cell(&side.slots[1]),
        );
    }
}
