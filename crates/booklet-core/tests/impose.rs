//! End-to-end kontrola: vytvor PDF, prepočítaj ho a preveď späť na
//! mapu „výstupná strana → zdrojové strany“.

use booklet_core::{impose_file, Flip, FoldMark, Marks, Mode, Options, Paper, Slot};
use lopdf::{Document, Object};

/// Zdrojové PDF s `n` stranami A4 portrait, kde n-tá strana obsahuje `PAGE<n>`.
fn source(n: usize) -> Document {
    use lopdf::{dictionary, Object, Stream};

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut kids = Vec::new();
    for i in 1..=n {
        let text = format!("BT /F1 24 Tf 72 700 Td (PAGE{i}) Tj ET");
        let content_id = doc.add_object(Stream::new(dictionary! {}, text.into_bytes()));
        kids.push(Object::Reference(doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        })));
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
            "Resources" => resources_id,
            "MediaBox" => vec![
                Object::Real(0.0), Object::Real(0.0),
                Object::Real(595.276), Object::Real(841.89),
            ],
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    doc
}

struct Out {
    doc: Document,
    /// Pre každú výstupnú stranu zoznam `(názov slotu, číslo zdrojovej strany)`.
    slots: Vec<Vec<(String, Option<usize>)>>,
}

/// Prepočíta dokument a zistí, ktorá zdrojová strana skončila v ktorom slote.
fn run(pages: usize, opts: &Options) -> Out {
    let dir = std::env::temp_dir().join(format!("booklet-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let stem = format!("{pages}-{}", unique());
    let src = dir.join(format!("src-{stem}.pdf"));
    let dst = dir.join(format!("out-{stem}.pdf"));
    source(pages).save(&src).unwrap();

    impose_file(&src, &dst, opts).unwrap();
    let doc = Document::load(&dst).unwrap();

    let mut slots = Vec::new();
    for page_id in doc.get_pages().into_values() {
        let content = String::from_utf8_lossy(&doc.get_page_content(page_id)).to_string();
        let (resources, _) = doc.get_page_resources(page_id).unwrap();
        let xobjects =
            resources.unwrap().get(b"XObject").and_then(Object::as_dict).unwrap().clone();

        // Slot názvy v poradí, v akom sa kreslia.
        let mut used: Vec<(String, Option<usize>)> = Vec::new();
        for name in ["X0", "X1"] {
            if !content.contains(&format!("/{name} Do")) {
                used.push((name.to_string(), None));
                continue;
            }
            let id = xobjects.get(name.as_bytes()).unwrap().as_reference().unwrap();
            let form = doc.get_object(id).unwrap().as_stream().unwrap();
            let body = String::from_utf8_lossy(&form.decompressed_content().unwrap()).to_string();
            let page = (1..=64).find(|i| body.contains(&format!("PAGE{i})")));
            used.push((name.to_string(), page));
        }
        slots.push(used);
    }
    std::fs::remove_file(&src).ok();
    std::fs::remove_file(&dst).ok();
    Out { doc, slots }
}

/// Jedinečná prípona — testy bežia paralelne a nesmú si prepisovať súbory.
fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// `a` a `d` z prvého operátora `cm` na danej výstupnej strane (1-based).
fn first_cm_diagonal(out: &Out, page_no: u32) -> Option<(f64, f64)> {
    let content = out.doc.get_page_content(out.doc.get_pages()[&page_no]);
    let content = String::from_utf8_lossy(&content);
    let line = content.lines().find(|l| l.contains(" cm "))?;
    let t: Vec<&str> = line.split_whitespace().collect();
    Some((t[1].parse().ok()?, t[4].parse().ok()?))
}

fn map(out: &Out) -> Vec<[Option<usize>; 2]> {
    out.slots.iter().map(|s| [s[0].1, s[1].1]).collect()
}

#[test]
fn booklet_of_eight_pages_lands_in_the_right_slots() {
    let out = run(8, &Options::default());
    assert_eq!(out.doc.get_pages().len(), 4);
    assert_eq!(
        map(&out),
        vec![[Some(8), Some(1)], [Some(2), Some(7)], [Some(6), Some(3)], [Some(4), Some(5)],]
    );
}

#[test]
fn blank_slots_stay_empty() {
    let out = run(5, &Options::default());
    // 5 strán -> 8 pozícií, 3 prázdne.
    let all: Vec<Option<usize>> = map(&out).into_iter().flatten().collect();
    assert_eq!(all.iter().filter(|p| p.is_none()).count(), 3);
    assert_eq!(all[1], Some(1), "prvá strana musí byť na prednej obálke");
}

#[test]
fn signatures_produce_independent_booklets() {
    let opts = Options {
        plan: booklet_core::PlanOptions {
            mode: Mode::Signatures { sheets: 1 },
            ..Default::default()
        },
        ..Options::default()
    };
    let out = run(8, &opts);
    assert_eq!(
        map(&out),
        vec![[Some(4), Some(1)], [Some(2), Some(3)], [Some(8), Some(5)], [Some(6), Some(7)],]
    );
}

#[test]
fn output_sheet_is_a4_landscape() {
    let out = run(4, &Options::default());
    let page_id = out.doc.get_pages()[&1];
    let media = out
        .doc
        .get_dictionary(page_id)
        .unwrap()
        .get(b"MediaBox")
        .and_then(Object::as_array)
        .unwrap();
    let w = media[2].as_float().unwrap();
    let h = media[3].as_float().unwrap();
    assert!((w - 841.89).abs() < 0.5, "šírka {w}");
    assert!((h - 595.28).abs() < 0.5, "výška {h}");
}

#[test]
fn paper_from_source_avoids_scaling() {
    let opts = Options { paper: Paper::FromSource, ..Options::default() };
    let out = run(4, &opts);
    let page_id = out.doc.get_pages()[&1];
    let media = out
        .doc
        .get_dictionary(page_id)
        .unwrap()
        .get(b"MediaBox")
        .and_then(Object::as_array)
        .unwrap();
    // Zdroj z lopdf je A4 portrait -> list má byť presne 2x na šírku.
    assert!((media[2].as_float().unwrap() - 2.0 * 595.276).abs() < 1.0);
}

#[test]
fn long_edge_flip_rotates_back_sides() {
    let opts = Options {
        plan: booklet_core::PlanOptions { flip: Flip::LongEdge, ..Default::default() },
        ..Options::default()
    };
    let out = run(4, &opts);
    assert_eq!(map(&out), vec![[Some(4), Some(1)], [Some(3), Some(2)]]);
    // Rub má otočenie o 180° -> záporná diagonála v matici cm, líce nie.
    assert_eq!(first_cm_diagonal(&out, 2).map(|(a, d)| (a < 0.0, d < 0.0)), Some((true, true)));
    assert_eq!(first_cm_diagonal(&out, 1).map(|(a, d)| (a < 0.0, d < 0.0)), Some((false, false)));
}

/// Grafické operátory na prvej výstupnej strane.
fn first_page_content(out: &Out) -> String {
    String::from_utf8_lossy(&out.doc.get_page_content(out.doc.get_pages()[&1])).to_string()
}

#[test]
fn marks_can_be_switched_off() {
    let out = run(4, &Options { marks: Marks::NONE, ..Options::default() });
    assert!(!first_page_content(&out).contains(" l S"));
}

#[test]
fn fold_ticks_stay_at_the_sheet_edges() {
    let out = run(
        4,
        &Options {
            marks: Marks { fold: FoldMark::Ticks, crop: false },
            margin_mm: 10.0,
            ..Options::default()
        },
    );
    let content = first_page_content(&out);
    // Dve krátke značky na osi listu (420.94 pt), nie čiara cez celú výšku.
    // Dĺžka je okraj obmedzený na 6 mm = 17.01 pt.
    assert!(content.contains("420.94488 0 m 420.94488 17.00787 l S"), "{content}");
    assert!(content.contains("420.94488 578.26772 m 420.94488 595.27559 l S"), "{content}");
    assert!(!content.contains("[4 4] 0 d"), "značky nesmú byť čiara: {content}");
}

#[test]
fn fold_line_crosses_the_whole_sheet() {
    let out = run(
        4,
        &Options { marks: Marks { fold: FoldMark::Line, crop: false }, ..Options::default() },
    );
    let content = first_page_content(&out);
    assert!(content.contains("[4 4] 0 d 420.94488 0 m 420.94488 595.27559 l S"), "{content}");
}

#[test]
fn crop_marks_are_independent_of_the_fold_mark() {
    let out = run(
        4,
        &Options { marks: Marks { fold: FoldMark::None, crop: true }, ..Options::default() },
    );
    let content = first_page_content(&out);
    assert!(!content.contains("[4 4] 0 d"), "prehyb sa nemá značiť: {content}");
    // Bez medzery v prehybe splynú vnútorné hrany strán: 3 svislé polohy × 2
    // hrany + 2 vodorovné polohy × 2 hrany = 10 značiek.
    assert_eq!(content.matches(" l S").count(), 10, "{content}");
}

#[test]
fn crop_marks_follow_the_gutter() {
    let out = run(
        4,
        &Options {
            marks: Marks { fold: FoldMark::None, crop: true },
            gutter_mm: 10.0,
            ..Options::default()
        },
    );
    // S medzerou sú vnútorné hrany strán rôzne: 4 svislé polohy × 2 + 2 × 2 = 12.
    assert_eq!(first_page_content(&out).matches(" l S").count(), 12);
}

#[test]
fn page_range_is_respected() {
    let opts = Options { range: "2-3".into(), ..Options::default() };
    let out = run(8, &opts);
    assert_eq!(map(&out), vec![[None, Some(2)], [Some(3), None]]);
}

#[test]
fn slot_default_is_blank() {
    assert_eq!(Slot::default().page, None);
}
