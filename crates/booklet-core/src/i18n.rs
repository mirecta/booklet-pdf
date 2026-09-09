//! Texty pre používateľské rozhrania v slovenčine, češtine a angličtine.
//!
//! Preklady sú zapísané tabuľkou — chýbajúci jazyk pri niektorom texte je
//! chyba prekladu, nie až chyba za behu. Reťazce s číslovkami sú vlastné
//! metódy, aby sa dalo skloňovať podľa jazyka.

use crate::geom::Paper;
use crate::plan::{Face, Mode};
use crate::{Error, RangeError};

/// Podporovaný jazyk rozhrania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Sk,
    Cs,
}

impl Lang {
    /// V poradí, v akom sa nabídnú v rozhraní.
    pub const ALL: &'static [Lang] = &[Lang::En, Lang::Sk, Lang::Cs];

    /// Kód jazyka podľa ISO 639-1.
    pub fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Sk => "sk",
            Lang::Cs => "cs",
        }
    }

    /// Názov jazyka v tom jazyku — tie sa neprekladajú.
    pub fn endonym(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Sk => "Slovenčina",
            Lang::Cs => "Čeština",
        }
    }

    /// Rozpozná jazyk z označenia typu `sk`, `cs_CZ.UTF-8` alebo `sk:en`.
    pub fn from_tag(tag: &str) -> Option<Lang> {
        let head = tag.trim().to_ascii_lowercase();
        let head = head.split([':', '.', '_', '-', '@']).next().unwrap_or("");
        match head {
            "sk" | "slk" | "slo" => Some(Lang::Sk),
            "cs" | "cz" | "ces" | "cze" => Some(Lang::Cs),
            "en" | "eng" => Some(Lang::En),
            _ => None,
        }
    }

    /// Jazyk podľa prostredia; `BOOKLET_LANG` má prednosť pred locale.
    /// Ak sa nič nerozpozná, angličtina.
    pub fn detect() -> Lang {
        Self::detect_from(|name| std::env::var(name).ok())
    }

    /// Detekcia s vlastným zdrojom premenných — kvôli testovateľnosti.
    pub fn detect_from(get: impl Fn(&str) -> Option<String>) -> Lang {
        for name in ["BOOKLET_LANG", "LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Some(value) = get(name) {
                if let Some(lang) = Lang::from_tag(&value) {
                    return lang;
                }
            }
        }
        Lang::En
    }
}

/// Vygeneruje metódu na každý text a vynúti všetky tri jazyky.
macro_rules! texts {
    ($( $(#[$doc:meta])* $name:ident = $en:literal | $sk:literal | $cs:literal; )*) => {
        impl Lang {
            $(
                $(#[$doc])*
                pub fn $name(self) -> &'static str {
                    match self {
                        Lang::En => $en,
                        Lang::Sk => $sk,
                        Lang::Cs => $cs,
                    }
                }
            )*
        }
    };
}

texts! {
    // ---- hlavičk a a dialógy ----
    open_tooltip = "Open PDF (Ctrl+O)" | "Otvoriť PDF (Ctrl+O)" | "Otevřít PDF (Ctrl+O)";
    save_button = "Save PDF…" | "Uložiť PDF…" | "Uložit PDF…";
    language_tooltip = "Interface language" | "Jazyk rozhrania" | "Jazyk rozhraní";
    dialog_open = "Open PDF" | "Otvoriť PDF" | "Otevřít PDF";
    dialog_save = "Save the imposed PDF" | "Uložiť prepočítané PDF" | "Uložit přepočítané PDF";
    dialog_error = "Could not process the PDF" | "Nepodarilo sa spracovať PDF"
        | "PDF se nepodařilo zpracovat";
    password_title = "Password" | "Heslo" | "Heslo";
    password_prompt = "This PDF is password protected." | "PDF je chránené heslom."
        | "PDF je chráněno heslem.";
    button_cancel = "Cancel" | "Zrušiť" | "Zrušit";
    button_open = "Open" | "Otvoriť" | "Otevřít";

    // ---- sekcie a riadky ----
    section_input = "Input" | "Vstup" | "Vstup";
    section_folding = "Folding" | "Skladanie" | "Skládání";
    section_paper = "Paper sheet" | "List papiera" | "List papíru";
    section_printing = "Printing" | "Tlač" | "Tisk";
    row_mode = "Mode" | "Režim" | "Režim";
    row_sheets = "Sheets per signature" | "Listov v zošite" | "Listů ve složce";
    row_binding = "Binding" | "Väzba" | "Vazba";
    row_pages = "Pages" | "Strany" | "Strany";
    row_paper = "Size" | "Formát" | "Formát";
    row_orientation = "Orientation" | "Orientácia" | "Orientace";
    row_margin = "Margin (mm)" | "Okraj (mm)" | "Okraj (mm)";
    row_gutter = "Fold gutter (mm)" | "Prehyb (mm)" | "Přehyb (mm)";
    row_fold = "Mark the fold" | "Prehyb značiť" | "Značit přehyb";
    row_flip = "Paper flip" | "Obrat papiera" | "Obracení papíru";
    row_order = "Order" | "Poradie" | "Pořadí";
    row_creep = "Creep (mm/sheet)" | "Creep (mm/list)" | "Creep (mm/list)";

    // ---- prepínače ----
    check_crop = "Crop marks at the edges" | "Orezové značky na hranách"
        | "Ořezové značky na hranách";
    check_scale = "Scale pages to fit the sheet" | "Prispôsobiť mierku na list"
        | "Přizpůsobit měřítko listu";
    check_thumbs = "Page thumbnails" | "Náhľady strán" | "Náhledy stran";

    // ---- hodnoty v rozbaľovacích zoznamoch ----
    mode_booklet = "Booklet – saddle stitch" | "Brožúra – zošitá v strede"
        | "Brožura – sešitá ve hřbetu";
    mode_signatures = "Signatures – folded in sections" | "Zošity – skladané po častiach"
        | "Složky – skládané po částech";
    mode_two_up = "2 pages per sheet – no folding" | "2 strany na list – bez skladania"
        | "2 strany na list – bez skládání";
    orientation_landscape = "Landscape" | "Na ležato" | "Na šířku";
    orientation_portrait = "Portrait" | "Na stojato" | "Na výšku";
    binding_left = "Left (normal)" | "Vľavo (bežné)" | "Vlevo (běžné)";
    binding_right = "Right (RTL, manga)" | "Vpravo (RTL, manga)" | "Vpravo (RTL, manga)";
    flip_short_edge = "on the short edge" | "po krátkej hrane" | "po krátké hraně";
    flip_long_edge = "on the long edge" | "po dlhej hrane" | "po dlouhé hraně";
    order_interleaved = "Duplex – fronts and backs interleaved" | "Duplex – líce/rub za sebou"
        | "Duplex – líc/rub za sebou";
    order_fronts_then_backs = "Manual duplex – all fronts first" | "Ručný duplex – najprv líca"
        | "Ruční duplex – nejdřív líce";
    order_backs_reversed = "Manual duplex – backs in reverse" | "Ručný duplex – ruby odzadu"
        | "Ruční duplex – ruby odzadu";
    fold_none = "Do not mark" | "Neznačiť" | "Neznačit";
    fold_ticks = "Ticks at the sheet edges" | "Značky pri hranách listu" | "Značky u hran listu";
    fold_line = "Dashed line across the sheet" | "Prerušovaná čiara cez list"
        | "Přerušovaná čára přes list";
    paper_from_source = "match the source" | "podľa zdroja" | "podle zdroje";

    // ---- vysvetlivky ----
    tip_flip = "Must match the duplex setting in the printer driver.\n\
                If the backs come out upside down, switch this."
        | "Musí sedieť s nastavením duplexu v ovládači tlačiarne.\n\
           Ak je rub hlavou dolu, prepni túto voľbu."
        | "Musí odpovídat nastavení duplexu v ovladači tiskárny.\n\
           Pokud je rub hlavou dolů, přepni tuto volbu.";
    tip_creep = "Shifts the content of outer sheets towards the fold so the margins\n\
                 come out even after trimming. Leave at 0 for thin booklets."
        | "Posunie obsah vonkajších listov k prehybu, aby po orezaní\n\
           vyšli okraje rovnako. Pri tenkých brožúrach nechaj 0."
        | "Posune obsah vnějších listů k přehybu, aby po ořezu\n\
           vyšly okraje stejně. U tenkých brožur nechej 0.";
    tip_fold = "Ticks at the edges show where to fold without drawing over the page\n\
                content. The dashed line is easier to see but stays printed."
        | "Značky pri hranách ukážu, kde list prehnúť, a nekreslia sa cez obsah strán.\n\
           Prerušovaná čiara je viditeľnejšia, ale zostane vytlačená v knižke."
        | "Značky u hran ukážou, kde list přehnout, a nekreslí se přes obsah stran.\n\
           Přerušovaná čára je vidět lépe, ale zůstane vytištěná v knížce.";
    tip_scale = "Off = scale 1:1, the content may not fit."
        | "Vypnuté = mierka 1:1, obsah sa môže nezmestiť."
        | "Vypnuto = měřítko 1:1, obsah se nemusí vejít.";
    tip_thumbs = "Draws the real page content under the numbers. Rendered in the\n\
                  background, and only what is currently visible."
        | "Vykreslí skutočný obsah strán pod čísla. Renderuje sa na pozadí\n\
           a len to, čo je práve vidno."
        | "Vykreslí skutečný obsah stran pod čísla. Renderuje se na pozadí\n\
           a jen to, co je právě vidět.";

    // ---- stav a náhľad ----
    no_file = "No file" | "Žiadny súbor" | "Žádný soubor";
    pages_placeholder = "all, e.g. 1-8,11" | "všetky, napr. 1-8,11" | "všechny, např. 1-8,11";
    status_open_pdf = "Open a PDF." | "Otvor PDF." | "Otevři PDF.";
    status_working = "Working…" | "Prepočítavam…" | "Přepočítávám…";
    preview_empty = "Open a PDF and the sheet layout will appear here."
        | "Otvor PDF a tu sa zobrazí rozloženie strán na listoch."
        | "Otevři PDF a tady se zobrazí rozložení stran na listech.";
    status_one_signature = "The whole document fits into a single signature, so the result\n\
                            is the same as a plain booklet. Reduce the sheets per signature."
        | "Celý dokument sa zmestí do jedného zošita, takže výsledok je\n\
           rovnaký ako pri brožúre. Zmenši počet listov v zošite."
        | "Celý dokument se vejde do jedné složky, takže výsledek je\n\
           stejný jako u brožury. Zmenši počet listů ve složce.";
    face_front = "front" | "líce" | "líc";
    face_back = "back" | "rub" | "rub";
    rotated_note = "rotated 180°" | "otočené 180°" | "otočeno o 180°";

    // ---- chyby ----
    error_encrypted = "The PDF is password protected" | "PDF je chránené heslom"
        | "PDF je chráněno heslem";
    error_no_pages = "The document contains no pages" | "Dokument neobsahuje žiadne strany"
        | "Dokument neobsahuje žádné strany";
    error_interrupted = "the imposition was interrupted unexpectedly"
        | "prepočet sa nečakane prerušil" | "přepočet se nečekaně přerušil";
    range_zero_page = "page numbers start at 1" | "čísla strán začínajú od 1"
        | "čísla stran začínají od 1";
    range_empty = "the range contains no pages" | "rozsah neobsahuje žiadnu stranu"
        | "rozsah neobsahuje žádnou stranu";
}

/// Skloňovanie podstatného mena po číslovke.
///
/// Slovenčina aj čeština potrebujú tri formy: 1, 2–4 a 5 a viac.
fn plural(lang: Lang, n: usize, forms: [&'static str; 3]) -> &'static str {
    match lang {
        Lang::En => {
            if n == 1 {
                forms[0]
            } else {
                forms[1]
            }
        }
        _ => match n {
            1 => forms[0],
            2..=4 => forms[1],
            _ => forms[2],
        },
    }
}

impl Lang {
    /// „page/pages“, „strana/strany/strán“, „strana/strany/stran“
    pub fn word_page(self, n: usize) -> &'static str {
        match self {
            Lang::En => plural(self, n, ["page", "pages", "pages"]),
            Lang::Sk => plural(self, n, ["strana", "strany", "strán"]),
            Lang::Cs => plural(self, n, ["strana", "strany", "stran"]),
        }
    }

    /// List papiera.
    pub fn word_sheet(self, n: usize) -> &'static str {
        match self {
            Lang::En => plural(self, n, ["sheet", "sheets", "sheets"]),
            Lang::Sk => plural(self, n, ["list", "listy", "listov"]),
            Lang::Cs => plural(self, n, ["list", "listy", "listů"]),
        }
    }

    /// Zošit (signatúra).
    pub fn word_signature(self, n: usize) -> &'static str {
        match self {
            Lang::En => plural(self, n, ["signature", "signatures", "signatures"]),
            Lang::Sk => plural(self, n, ["zošit", "zošity", "zošitov"]),
            Lang::Cs => plural(self, n, ["složka", "složky", "složek"]),
        }
    }

    /// Prázdne miesto na liste.
    pub fn word_blank(self, n: usize) -> &'static str {
        match self {
            Lang::En => plural(self, n, ["blank slot", "blank slots", "blank slots"]),
            Lang::Sk => plural(self, n, ["prázdne miesto", "prázdne miesta", "prázdnych miest"]),
            Lang::Cs => plural(self, n, ["prázdné místo", "prázdná místa", "prázdných míst"]),
        }
    }

    /// Názov formátu papiera. Formáty ako `A4` sa neprekládajú.
    pub fn paper_label(self, paper: Paper) -> String {
        match paper {
            Paper::FromSource => self.paper_from_source().to_string(),
            Paper::Custom { w_mm, h_mm } => format!("{w_mm:.0}×{h_mm:.0} mm"),
            other => other.name().to_string(),
        }
    }

    /// Názov režimu skladania.
    pub fn mode_label(self, mode: Mode) -> &'static str {
        match mode {
            Mode::Booklet => self.mode_booklet(),
            Mode::Signatures { .. } => self.mode_signatures(),
            Mode::TwoUp => self.mode_two_up(),
        }
    }

    pub fn face_label(self, face: Face) -> &'static str {
        match face {
            Face::Front => self.face_front(),
            Face::Back => self.face_back(),
        }
    }

    /// „14 pages, first page 210×297 mm“
    pub fn file_info(self, pages: usize, w_mm: f64, h_mm: f64) -> String {
        let word = self.word_page(pages);
        match self {
            Lang::En => format!("{pages} {word}, first page {w_mm:.0}×{h_mm:.0} mm"),
            Lang::Sk => format!("{pages} {word}, prvá strana {w_mm:.0}×{h_mm:.0} mm"),
            Lang::Cs => format!("{pages} {word}, první strana {w_mm:.0}×{h_mm:.0} mm"),
        }
    }

    /// „40 source pages → 10 sheets of paper (20 output pages), 0 blank slots.“
    pub fn counts(self, pages: usize, sheets: usize, output: usize, blanks: usize) -> String {
        let (p, s, o, b) = (
            self.word_page(pages),
            self.word_sheet(sheets),
            self.word_page(output),
            self.word_blank(blanks),
        );
        match self {
            Lang::En => format!(
                "{pages} source {p} → {sheets} {s} of paper ({output} output {o}), {blanks} {b}."
            ),
            Lang::Sk => format!(
                "{pages} {p} zdroja → {sheets} {s} papiera ({output} {o} výstupu), {blanks} {b}."
            ),
            Lang::Cs => format!(
                "{pages} {p} zdroje → {sheets} {s} papíru ({output} {o} výstupu), {blanks} {b}."
            ),
        }
    }

    /// „3 signatures, sheets: 4+4+2“
    pub fn signatures_summary(self, count: usize, breakdown: &str) -> String {
        let word = self.word_signature(count);
        match self {
            Lang::En => format!("{count} {word}, sheets: {breakdown}"),
            Lang::Sk => format!("{count} {word}, listov po {breakdown}"),
            Lang::Cs => format!("{count} {word}, listů po {breakdown}"),
        }
    }

    /// „Sheet 297×210 mm.“
    pub fn sheet_size(self, w_mm: f64, h_mm: f64) -> String {
        match self {
            Lang::En => format!("Sheet {w_mm:.0}×{h_mm:.0} mm."),
            Lang::Sk => format!("List {w_mm:.0}×{h_mm:.0} mm."),
            Lang::Cs => format!("List {w_mm:.0}×{h_mm:.0} mm."),
        }
    }

    /// Hlásenie po uložení výstupu.
    pub fn saved(self, path: &str, sheets: usize, output: usize) -> String {
        let (s, o) = (self.word_sheet(sheets), self.word_page(output));
        match self {
            Lang::En => format!("Saved: {path}\n{sheets} {s}, {output} output {o}."),
            Lang::Sk => format!("Uložené: {path}\n{sheets} {s}, {output} {o} výstupu."),
            Lang::Cs => format!("Uloženo: {path}\n{sheets} {s}, {output} {o} výstupu."),
        }
    }

    /// Nadpis zošita v náhľade: „Signature 1 of 3 — 4 sheets“
    pub fn signature_heading(self, index: usize, total: usize, sheets: usize) -> String {
        let word = self.word_sheet(sheets);
        match self {
            Lang::En => format!("Signature {index} of {total} — {sheets} {word}"),
            Lang::Sk => format!("Zošit {index} z {total} — {sheets} {word}"),
            Lang::Cs => format!("Složka {index} z {total} — {sheets} {word}"),
        }
    }

    /// Popis strany výstupu nad listom v náhľade.
    ///
    /// `sheet_in_signature` je 1-based poradie listu v zošite; `None`, keď
    /// dokument nie je rozdelený na viac zošitov.
    pub fn side_label(
        self,
        output_page: usize,
        sheet: usize,
        face: Face,
        sheet_in_signature: Option<usize>,
    ) -> String {
        let face = self.face_label(face);
        let mut label = match self {
            Lang::En => format!("output page {output_page} — sheet {sheet} {face}"),
            Lang::Sk => format!("{output_page}. strana výstupu — list {sheet} {face}"),
            Lang::Cs => format!("{output_page}. strana výstupu — list {sheet} {face}"),
        };
        if let Some(n) = sheet_in_signature {
            label.push_str(&match self {
                Lang::En => format!(" (sheet {n} of the signature)"),
                Lang::Sk => format!(" ({n}. list zošita)"),
                Lang::Cs => format!(" ({n}. list složky)"),
            });
        }
        label
    }

    /// Preložené hlásenie chyby.
    pub fn error(self, error: &Error) -> String {
        match error {
            Error::Encrypted => self.error_encrypted().to_string(),
            Error::NoPages => self.error_no_pages().to_string(),
            Error::Range(range) => {
                let detail = self.range_error(range);
                match self {
                    Lang::En => format!("Invalid page range: {detail}"),
                    Lang::Sk => format!("Chybný rozsah strán: {detail}"),
                    Lang::Cs => format!("Chybný rozsah stran: {detail}"),
                }
            }
            // Podrobnosti od lopdf a operačného systému sú po anglicky.
            Error::Pdf(detail) => match self {
                Lang::En => format!("Could not read the PDF: {detail}"),
                Lang::Sk => format!("PDF sa nepodarilo prečítať: {detail}"),
                Lang::Cs => format!("PDF se nepodařilo přečíst: {detail}"),
            },
            Error::Io(detail) => detail.to_string(),
        }
    }

    /// Podrobnosť k chybnému rozsahu strán.
    pub fn range_error(self, error: &RangeError) -> String {
        match error {
            RangeError::ZeroPage => self.range_zero_page().to_string(),
            RangeError::Empty => self.range_empty().to_string(),
            RangeError::NotANumber(text) => match self {
                Lang::En => format!("not a page number: {text:?}"),
                Lang::Sk => format!("neplatné číslo strany: {text:?}"),
                Lang::Cs => format!("neplatné číslo strany: {text:?}"),
            },
            RangeError::OutOfBounds { page, total } => {
                let word = self.word_page(*total);
                match self {
                    Lang::En => {
                        format!("page {page} is beyond the document ({total} {word})")
                    }
                    Lang::Sk => format!("strana {page} presahuje dokument ({total} {word})"),
                    Lang::Cs => format!("strana {page} přesahuje dokument ({total} {word})"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_tag(lang.tag()), Some(*lang));
        }
    }

    #[test]
    fn locale_strings_are_recognised() {
        assert_eq!(Lang::from_tag("sk_SK.UTF-8"), Some(Lang::Sk));
        assert_eq!(Lang::from_tag("cs_CZ.UTF-8"), Some(Lang::Cs));
        assert_eq!(Lang::from_tag("cz"), Some(Lang::Cs));
        assert_eq!(Lang::from_tag("en_GB"), Some(Lang::En));
        assert_eq!(Lang::from_tag("sk:en"), Some(Lang::Sk));
        assert_eq!(Lang::from_tag("C"), None);
        assert_eq!(Lang::from_tag("de_DE.UTF-8"), None);
    }

    #[test]
    fn detection_prefers_our_own_variable() {
        let env = |name: &str| match name {
            "BOOKLET_LANG" => Some("cs".to_string()),
            "LANG" => Some("sk_SK.UTF-8".to_string()),
            _ => None,
        };
        assert_eq!(Lang::detect_from(env), Lang::Cs);
    }

    #[test]
    fn detection_skips_unknown_locales_and_falls_back_to_english() {
        let env = |name: &str| match name {
            "LC_ALL" => Some("C".to_string()),
            "LANG" => Some("de_DE.UTF-8".to_string()),
            _ => None,
        };
        assert_eq!(Lang::detect_from(env), Lang::En);
        assert_eq!(Lang::detect_from(|_| None), Lang::En);
    }

    #[test]
    fn slavic_plurals_have_three_forms() {
        assert_eq!(Lang::Sk.word_sheet(1), "list");
        assert_eq!(Lang::Sk.word_sheet(3), "listy");
        assert_eq!(Lang::Sk.word_sheet(10), "listov");
        assert_eq!(Lang::Cs.word_signature(1), "složka");
        assert_eq!(Lang::Cs.word_signature(2), "složky");
        assert_eq!(Lang::Cs.word_signature(9), "složek");
    }

    #[test]
    fn english_plurals_have_two_forms() {
        assert_eq!(Lang::En.word_sheet(1), "sheet");
        assert_eq!(Lang::En.word_sheet(2), "sheets");
        assert_eq!(Lang::En.word_sheet(10), "sheets");
    }

    #[test]
    fn counts_read_naturally_in_every_language() {
        assert_eq!(
            Lang::En.counts(40, 10, 20, 0),
            "40 source pages → 10 sheets of paper (20 output pages), 0 blank slots."
        );
        assert_eq!(
            Lang::Sk.counts(1, 1, 2, 3),
            "1 strana zdroja → 1 list papiera (2 strany výstupu), 3 prázdne miesta."
        );
        assert_eq!(
            Lang::Cs.counts(14, 4, 8, 2),
            "14 stran zdroje → 4 listy papíru (8 stran výstupu), 2 prázdná místa."
        );
    }

    #[test]
    fn signature_labels_use_the_right_term() {
        assert_eq!(Lang::En.signature_heading(1, 3, 4), "Signature 1 of 3 — 4 sheets");
        assert_eq!(Lang::Sk.signature_heading(1, 3, 4), "Zošit 1 z 3 — 4 listy");
        assert_eq!(Lang::Cs.signature_heading(3, 3, 1), "Složka 3 z 3 — 1 list");
    }

    #[test]
    fn side_label_mentions_the_signature_only_when_grouped() {
        assert_eq!(Lang::En.side_label(1, 1, Face::Front, None), "output page 1 — sheet 1 front");
        assert_eq!(
            Lang::En.side_label(9, 5, Face::Back, Some(1)),
            "output page 9 — sheet 5 back (sheet 1 of the signature)"
        );
    }

    #[test]
    fn paper_names_are_not_translated_but_special_values_are() {
        assert_eq!(Lang::Sk.paper_label(Paper::A4), "A4");
        assert_eq!(Lang::Sk.paper_label(Paper::FromSource), "podľa zdroja");
        assert_eq!(Lang::En.paper_label(Paper::FromSource), "match the source");
        assert_eq!(Lang::Cs.paper_label(Paper::Custom { w_mm: 200.0, h_mm: 250.0 }), "200×250 mm");
    }

    #[test]
    fn errors_are_translated() {
        assert_eq!(Lang::En.error(&Error::NoPages), "The document contains no pages");
        assert_eq!(Lang::Cs.error(&Error::Encrypted), "PDF je chráněno heslem");
        let range = Error::Range(RangeError::OutOfBounds { page: 9, total: 4 });
        assert_eq!(
            Lang::En.error(&range),
            "Invalid page range: page 9 is beyond the document (4 pages)"
        );
        assert_eq!(
            Lang::Sk.error(&range),
            "Chybný rozsah strán: strana 9 presahuje dokument (4 strany)"
        );
    }
}
