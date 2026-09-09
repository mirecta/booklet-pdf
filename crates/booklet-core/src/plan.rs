//! Plánovač impozície — čistá logika, nezávislá od PDF.
//!
//! Výsledkom je zoznam *strán výstupu* ([`Side`]), každá s dvomi slotmi
//! (vľavo/vpravo). Plán sa dá vykresliť do PDF alebo zobraziť ako náhľad.

/// Režim skladania.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Jedna brožúra: všetky listy sa vložia do seba a zošijú v strede
    /// (V1 / saddle stitch). Na prvom liste je zvonku prvá a posledná strana.
    Booklet,
    /// Zošity (signatúry) po `sheets` listoch. Každý zošit sa poskladá zvlášť,
    /// zošity sa potom zošijú/zlepia za sebou — vhodné pre hrubé knihy.
    Signatures { sheets: usize },
    /// Bez skladania: 2 strany na list v pôvodnom poradí (1|2, 3|4, …).
    TwoUp,
}

impl Mode {
    pub fn label(&self) -> String {
        match self {
            Mode::Booklet => "Brožúra (zošitá v strede)".into(),
            Mode::Signatures { sheets } => format!("Zošity po {sheets} listoch"),
            Mode::TwoUp => "2 strany na list (bez skladania)".into(),
        }
    }
}

/// Na ktorej strane listu je väzba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// Bežné poradie čítania zľava doprava.
    Left,
    /// Pre jazyky písané sprava doľava (arabčina, hebrejčina, manga).
    Right,
}

/// Ako tlačiareň obracia papier pri obojstrannej tlači.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flip {
    /// Obrat okolo krátkej hrany. Pre list na ležato je to správna voľba
    /// (v ovládači tlače často „preklopiť po krátkej hrane“ / „book“).
    ShortEdge,
    /// Obrat okolo dlhej hrany — rub sa musí otočiť o 180°.
    LongEdge,
}

/// Poradie strán vo výstupnom PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetOrder {
    /// líce, rub, líce, rub … — pre tlačiarne s duplexom.
    Interleaved,
    /// najprv všetky líca, potom všetky ruby — pre ručný duplex.
    FrontsThenBacks,
    /// najprv všetky líca, potom ruby v opačnom poradí — ak tlačiareň
    /// vracia stoh obrátený.
    FrontsThenBacksReversed,
}

/// Líce alebo rub listu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Front,
    Back,
}

/// Jedno miesto na výstupnom liste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Slot {
    /// Index strany v zdrojovom dokumente (0-based). `None` = prázdne miesto.
    pub page: Option<usize>,
    pub rotate180: bool,
}

impl Slot {
    fn of(page: Option<usize>) -> Slot {
        Slot { page, rotate180: false }
    }
}

/// Jedna strana výstupného PDF = jedna strana jedného listu papiera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Side {
    /// Index listu papiera v rámci celého výstupu (0-based).
    pub sheet: usize,
    /// Index zošita (0-based). Pri [`Mode::Booklet`] vždy 0.
    pub signature: usize,
    /// Poradie listu vnútri zošita (0 = najvonkajší list).
    pub sheet_in_signature: usize,
    /// Počet listov v tomto zošite.
    pub sheets_in_signature: usize,
    pub face: Face,
    /// `[vľavo, vpravo]` v poradí ako sa vytlačia.
    pub slots: [Slot; 2],
}

/// Hotový plán impozície.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub sides: Vec<Side>,
    /// Počet listov papiera.
    pub sheets: usize,
    /// Počet zdrojových strán, ktoré do plánu vstúpili.
    pub source_pages: usize,
    /// Počet doplnených prázdnych miest.
    pub blanks: usize,
}

impl Plan {
    /// Počet strán výstupného PDF.
    pub fn output_pages(&self) -> usize {
        self.sides.len()
    }
}

/// Nastavenia, ktoré ovplyvňujú poradie strán (nie geometriu).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanOptions {
    pub mode: Mode,
    pub binding: Binding,
    pub flip: Flip,
    pub order: SheetOrder,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            mode: Mode::Booklet,
            binding: Binding::Left,
            flip: Flip::ShortEdge,
            order: SheetOrder::Interleaved,
        }
    }
}

/// Vytvorí plán pre zoznam zdrojových strán (indexy do pôvodného dokumentu).
pub fn plan(pages: &[usize], opts: &PlanOptions) -> Plan {
    let mut sides = Vec::new();
    let mut blanks = 0usize;
    let mut sheet = 0usize;

    match opts.mode {
        Mode::TwoUp => {
            let mut padded: Vec<Option<usize>> = pages.iter().map(|p| Some(*p)).collect();
            while !padded.len().is_multiple_of(2) {
                padded.push(None);
                blanks += 1;
            }
            let total_sides = padded.len() / 2;
            let sheets_total = total_sides.div_ceil(2);
            for (i, pair) in padded.chunks(2).enumerate() {
                let face = if i.is_multiple_of(2) { Face::Front } else { Face::Back };
                sides.push(Side {
                    sheet: i / 2,
                    signature: 0,
                    sheet_in_signature: i / 2,
                    sheets_in_signature: sheets_total,
                    face,
                    slots: [Slot::of(pair[0]), Slot::of(pair[1])],
                });
            }
            sheet = sheets_total;
        }
        Mode::Booklet | Mode::Signatures { .. } => {
            let per_signature = match opts.mode {
                Mode::Signatures { sheets } => 4 * sheets.max(1),
                _ => usize::MAX,
            };

            let mut padded: Vec<Option<usize>> = pages.iter().map(|p| Some(*p)).collect();
            while !padded.len().is_multiple_of(4) {
                padded.push(None);
                blanks += 1;
            }

            let groups: Vec<&[Option<usize>]> = if per_signature == usize::MAX {
                if padded.is_empty() {
                    vec![]
                } else {
                    vec![&padded[..]]
                }
            } else {
                padded.chunks(per_signature).collect()
            };

            for (sig, group) in groups.iter().enumerate() {
                let n = group.len();
                let sheets_in_sig = n / 4;
                for k in 0..sheets_in_sig {
                    // Klasické poradie brožúry: na líci k-tého listu je zvonku
                    // (n-2k)-tá a (2k+1)-vá strana zošita.
                    let front = [group[n - 1 - 2 * k], group[2 * k]];
                    let back = [group[2 * k + 1], group[n - 2 - 2 * k]];
                    for (face, pair) in [(Face::Front, front), (Face::Back, back)] {
                        sides.push(Side {
                            sheet,
                            signature: sig,
                            sheet_in_signature: k,
                            sheets_in_signature: sheets_in_sig,
                            face,
                            slots: [Slot::of(pair[0]), Slot::of(pair[1])],
                        });
                    }
                    sheet += 1;
                }
            }
        }
    }

    // Väzba vpravo = zrkadlové poradie slotov.
    if opts.binding == Binding::Right {
        for side in &mut sides {
            side.slots.swap(0, 1);
        }
    }

    // Kompenzácia obracania papiera okolo dlhej hrany.
    if opts.flip == Flip::LongEdge {
        for side in &mut sides {
            if side.face == Face::Back {
                side.slots.swap(0, 1);
                for slot in &mut side.slots {
                    slot.rotate180 = !slot.rotate180;
                }
            }
        }
    }

    let sides = reorder(sides, opts.order);
    let source_pages = pages.len();
    Plan { sides, sheets: sheet, source_pages, blanks }
}

fn reorder(sides: Vec<Side>, order: SheetOrder) -> Vec<Side> {
    match order {
        SheetOrder::Interleaved => sides,
        SheetOrder::FrontsThenBacks | SheetOrder::FrontsThenBacksReversed => {
            let (fronts, mut backs): (Vec<Side>, Vec<Side>) =
                sides.into_iter().partition(|s| s.face == Face::Front);
            if order == SheetOrder::FrontsThenBacksReversed {
                backs.reverse();
            }
            let mut out = fronts;
            out.extend(backs);
            out
        }
    }
}

/// Rozparsuje rozsah strán typu `"1-4,7,9-"` na 0-based indexy.
///
/// `total` je počet strán dokumentu. Prázdny/`None` vstup = všetky strany.
pub fn parse_range(spec: &str, total: usize) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok((0..total).collect());
    }
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (from, to) = match part.split_once('-') {
            None => {
                let n = parse_page(part, total)?;
                (n, n)
            }
            Some((a, b)) => {
                let a = if a.trim().is_empty() { 1 } else { parse_page(a, total)? };
                let b = if b.trim().is_empty() { total } else { parse_page(b, total)? };
                (a, b)
            }
        };
        if from == 0 || to == 0 {
            return Err("čísla strán začínajú od 1".into());
        }
        if from <= to {
            out.extend((from..=to).map(|n| n - 1));
        } else {
            out.extend((to..=from).rev().map(|n| n - 1));
        }
    }
    if out.is_empty() {
        return Err("rozsah neobsahuje žiadnu stranu".into());
    }
    Ok(out)
}

fn parse_page(s: &str, total: usize) -> Result<usize, String> {
    let n: usize = s.trim().parse().map_err(|_| format!("neplatné číslo strany: {s:?}"))?;
    if n > total {
        return Err(format!("strana {n} presahuje dokument ({total} strán)"));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(plan: &Plan) -> Vec<(Face, [Option<usize>; 2])> {
        plan.sides.iter().map(|s| (s.face, [s.slots[0].page, s.slots[1].page])).collect()
    }

    fn opts(mode: Mode) -> PlanOptions {
        PlanOptions { mode, ..Default::default() }
    }

    #[test]
    fn booklet_four_pages() {
        let p = plan(&[0, 1, 2, 3], &opts(Mode::Booklet));
        assert_eq!(p.sheets, 1);
        assert_eq!(
            pairs(&p),
            vec![(Face::Front, [Some(3), Some(0)]), (Face::Back, [Some(1), Some(2)]),]
        );
    }

    #[test]
    fn booklet_eight_pages() {
        let p = plan(&(0..8).collect::<Vec<_>>(), &opts(Mode::Booklet));
        assert_eq!(p.sheets, 2);
        assert_eq!(
            pairs(&p),
            vec![
                (Face::Front, [Some(7), Some(0)]),
                (Face::Back, [Some(1), Some(6)]),
                (Face::Front, [Some(5), Some(2)]),
                (Face::Back, [Some(3), Some(4)]),
            ]
        );
    }

    #[test]
    fn booklet_pads_to_multiple_of_four_at_the_back() {
        let p = plan(&[0, 1, 2, 3, 4, 5], &opts(Mode::Booklet));
        assert_eq!(p.blanks, 2);
        // Prvá strana zdroja musí zostať prednou obálkou.
        assert_eq!(p.sides[0].slots[1].page, Some(0));
        // Prázdne miesta padnú na zadnú obálku a jej vnútro.
        assert_eq!(p.sides[0].slots[0].page, None);
        assert_eq!(p.sides[1].slots[1].page, None);
    }

    #[test]
    fn signatures_split_into_independent_booklets() {
        let p = plan(&(0..16).collect::<Vec<_>>(), &opts(Mode::Signatures { sheets: 2 }));
        assert_eq!(p.sheets, 4);
        // Prvý zošit = strany 1..8, druhý = 9..16.
        assert_eq!(
            pairs(&p),
            vec![
                (Face::Front, [Some(7), Some(0)]),
                (Face::Back, [Some(1), Some(6)]),
                (Face::Front, [Some(5), Some(2)]),
                (Face::Back, [Some(3), Some(4)]),
                (Face::Front, [Some(15), Some(8)]),
                (Face::Back, [Some(9), Some(14)]),
                (Face::Front, [Some(13), Some(10)]),
                (Face::Back, [Some(11), Some(12)]),
            ]
        );
        assert_eq!(p.sides[4].signature, 1);
        assert_eq!(p.sides[4].sheet_in_signature, 0);
    }

    #[test]
    fn last_signature_may_be_shorter() {
        let p = plan(&(0..12).collect::<Vec<_>>(), &opts(Mode::Signatures { sheets: 2 }));
        assert_eq!(p.sheets, 3);
        assert_eq!(p.sides.last().unwrap().signature, 1);
        assert_eq!(p.sides.last().unwrap().sheets_in_signature, 1);
    }

    #[test]
    fn two_up_is_sequential() {
        let p = plan(&(0..5).collect::<Vec<_>>(), &opts(Mode::TwoUp));
        assert_eq!(
            pairs(&p),
            vec![
                (Face::Front, [Some(0), Some(1)]),
                (Face::Back, [Some(2), Some(3)]),
                (Face::Front, [Some(4), None]),
            ]
        );
    }

    #[test]
    fn binding_right_mirrors_slots() {
        let p =
            plan(&[0, 1, 2, 3], &PlanOptions { binding: Binding::Right, ..opts(Mode::Booklet) });
        assert_eq!(
            pairs(&p),
            vec![(Face::Front, [Some(0), Some(3)]), (Face::Back, [Some(2), Some(1)]),]
        );
    }

    #[test]
    fn long_edge_flip_rotates_backs() {
        let p = plan(&[0, 1, 2, 3], &PlanOptions { flip: Flip::LongEdge, ..opts(Mode::Booklet) });
        let back = p.sides[1];
        assert_eq!([back.slots[0].page, back.slots[1].page], [Some(2), Some(1)]);
        assert!(back.slots.iter().all(|s| s.rotate180));
        // Líce sa nemení.
        assert!(p.sides[0].slots.iter().all(|s| !s.rotate180));
    }

    #[test]
    fn fronts_then_backs_groups_faces() {
        let p = plan(
            &(0..8).collect::<Vec<_>>(),
            &PlanOptions { order: SheetOrder::FrontsThenBacks, ..opts(Mode::Booklet) },
        );
        let faces: Vec<Face> = p.sides.iter().map(|s| s.face).collect();
        assert_eq!(faces, vec![Face::Front, Face::Front, Face::Back, Face::Back]);
        assert_eq!(p.sides[2].slots[0].page, Some(1));
    }

    #[test]
    fn fronts_then_backs_reversed() {
        let p = plan(
            &(0..8).collect::<Vec<_>>(),
            &PlanOptions { order: SheetOrder::FrontsThenBacksReversed, ..opts(Mode::Booklet) },
        );
        assert_eq!(p.sides[2].slots[0].page, Some(3));
        assert_eq!(p.sides[3].slots[0].page, Some(1));
    }

    #[test]
    fn every_source_page_appears_exactly_once() {
        for n in 1..40usize {
            for mode in [Mode::Booklet, Mode::Signatures { sheets: 3 }, Mode::TwoUp] {
                let p = plan(&(0..n).collect::<Vec<_>>(), &opts(mode));
                let mut seen: Vec<usize> =
                    p.sides.iter().flat_map(|s| s.slots).filter_map(|s| s.page).collect();
                seen.sort_unstable();
                assert_eq!(seen, (0..n).collect::<Vec<_>>(), "n={n} mode={mode:?}");
            }
        }
    }

    #[test]
    fn empty_input_yields_empty_plan() {
        let p = plan(&[], &opts(Mode::Booklet));
        assert!(p.sides.is_empty());
        assert_eq!(p.sheets, 0);
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("", 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(parse_range("1-3", 5).unwrap(), vec![0, 1, 2]);
        assert_eq!(parse_range("2, 4", 5).unwrap(), vec![1, 3]);
        assert_eq!(parse_range("3-", 4).unwrap(), vec![2, 3]);
        assert_eq!(parse_range("-2", 4).unwrap(), vec![0, 1]);
        assert_eq!(parse_range("3-1", 4).unwrap(), vec![2, 1, 0]);
        assert!(parse_range("9", 4).is_err());
        assert!(parse_range("x", 4).is_err());
    }
}
