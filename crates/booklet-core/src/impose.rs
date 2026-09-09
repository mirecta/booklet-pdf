//! Prepis PDF podľa plánu impozície.
//!
//! Každá zdrojová strana sa zabalí do Form XObjectu (obsah aj `/Resources`
//! zostávajú pôvodné, takže fonty a obrázky sa neprekódovávajú) a potom sa
//! dva takéto XObjecty vykreslia na nový list.

use std::collections::BTreeMap;

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};

use crate::geom::{self, mm, num, Matrix, Orientation, Paper, Rect};
use crate::plan::{self, Plan, PlanOptions, Side};
use crate::{Error, Result};

/// Doplnkové značky na výstupnom liste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marks {
    None,
    /// Prerušovaná čiara v mieste prehybu.
    Fold,
    /// Prehyb + orezové značky na hranách listu.
    FoldAndCrop,
}

/// Kompletné nastavenie prepočtu.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub plan: PlanOptions,
    pub paper: Paper,
    pub orientation: Orientation,
    /// Okraj listu v mm (na všetkých štyroch stranách).
    pub margin_mm: f64,
    /// Extra medzera v mieste prehybu v mm (rozdelí sa na obe polovice).
    pub gutter_mm: f64,
    /// Kompenzácia posunu listov pri skladaní (creep) v mm na list.
    /// Vonkajšie listy sa posunú k prehybu. 0 = vypnuté.
    pub creep_mm: f64,
    /// Zmenšiť/zväčšiť stranu na slot. `false` = mierka 1:1.
    pub scale: bool,
    pub marks: Marks,
    /// Rozsah strán, napr. `"1-8,11"`. Prázdne = celý dokument.
    pub range: String,
    /// Heslo, ak je PDF zašifrované.
    pub password: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            plan: PlanOptions::default(),
            paper: Paper::A4,
            orientation: Orientation::Landscape,
            margin_mm: 0.0,
            gutter_mm: 0.0,
            creep_mm: 0.0,
            scale: true,
            marks: Marks::Fold,
            range: String::new(),
            password: None,
        }
    }
}

/// Základné údaje o vstupnom PDF.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfInfo {
    pub pages: usize,
    /// Efektívny rozmer prvej strany v bodoch (po zapečení `/Rotate`).
    pub first_page_pt: (f64, f64),
    pub encrypted: bool,
}

/// Súhrn vykonanej impozície.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub plan: Plan,
    /// Rozmer výstupného listu v bodoch.
    pub sheet_pt: (f64, f64),
}

/// Otvorí PDF a načíta metadáta.
pub fn open(path: &std::path::Path, password: Option<&str>) -> Result<Document> {
    let mut doc = match password {
        Some(p) => Document::load_with_password(path, p),
        None => Document::load(path),
    }
    .map_err(|e| match e {
        lopdf::Error::Decryption(_) => Error::Encrypted,
        other => Error::Pdf(other),
    })?;
    if doc.is_encrypted() {
        doc.decrypt(password.unwrap_or("")).map_err(|_| Error::Encrypted)?;
    }
    Ok(doc)
}

/// Načíta informácie o PDF bez akejkoľvek úpravy.
pub fn info(path: &std::path::Path, password: Option<&str>) -> Result<PdfInfo> {
    let doc = open(path, password)?;
    let ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
    let first = ids
        .first()
        .map(|id| {
            let (_, w, h) = page_geometry(&doc, *id);
            (w, h)
        })
        .unwrap_or((mm(210.0), mm(297.0)));
    Ok(PdfInfo { pages: ids.len(), first_page_pt: first, encrypted: doc.was_encrypted() })
}

/// Prepočíta PDF zo `input` do `output` podľa `opts`.
pub fn impose_file(
    input: &std::path::Path,
    output: &std::path::Path,
    opts: &Options,
) -> Result<Summary> {
    let mut doc = open(input, opts.password.as_deref())?;
    let summary = impose(&mut doc, opts)?;
    doc.save(output)?;
    Ok(summary)
}

/// Prepočíta už načítaný dokument na mieste.
pub fn impose(doc: &mut Document, opts: &Options) -> Result<Summary> {
    let source_ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
    if source_ids.is_empty() {
        return Err(Error::NoPages);
    }
    let selection = plan::parse_range(&opts.range, source_ids.len()).map_err(Error::Range)?;
    let plan = plan::plan(&selection, &opts.plan);
    if plan.sides.is_empty() {
        return Err(Error::NoPages);
    }

    // Rozmer výstupného listu.
    let (first_w, first_h) = {
        let (_, w, h) = page_geometry(doc, source_ids[0]);
        (w, h)
    };
    let sheet = match opts.paper.portrait_pt() {
        Some(p) => opts.orientation.apply(p),
        // „Podľa zdroja“: dve strany presne vedľa seba, plus okraje a prehyb.
        None => (
            2.0 * first_w + 2.0 * mm(opts.margin_mm) + mm(opts.gutter_mm),
            first_h + 2.0 * mm(opts.margin_mm),
        ),
    };

    // Form XObject pre každú použitú zdrojovú stranu (raz, aj keď sa opakuje).
    let mut forms: BTreeMap<usize, FormRef> = BTreeMap::new();
    for &page in &selection {
        if forms.contains_key(&page) {
            continue;
        }
        let form = make_form(doc, source_ids[page])?;
        forms.insert(page, form);
    }

    let pages_id = doc.new_object_id();
    let mut kids: Vec<Object> = Vec::with_capacity(plan.sides.len());
    for side in &plan.sides {
        let (content, xobjects) = compose_side(side, &forms, sheet, opts);
        let content_id = doc.add_object(Stream::new(dictionary! {}, content));
        let page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => dictionary! { "XObject" => xobjects },
            "MediaBox" => vec![Object::Real(0.0), Object::Real(0.0), Object::Real(sheet.0 as f32), Object::Real(sheet.1 as f32)],
            "Rotate" => 0,
        };
        kids.push(doc.add_object(page).into());
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
            "MediaBox" => vec![Object::Real(0.0), Object::Real(0.0), Object::Real(sheet.0 as f32), Object::Real(sheet.1 as f32)],
        }),
    );

    let catalog = doc.catalog_mut()?;
    catalog.set("Pages", pages_id);
    // Štruktúry viazané na staré strany by po prepise ukazovali do prázdna.
    for key in ["Outlines", "Names", "StructTreeRoot", "AcroForm", "Threads", "PageLabels", "Dests"]
    {
        catalog.remove(key.as_bytes());
    }
    doc.change_producer("booklet-pdf");
    doc.prune_objects();
    doc.renumber_objects();
    doc.compress();

    Ok(Summary { plan, sheet_pt: sheet })
}

/// Odkaz na pripravený Form XObject vrátane efektívnych rozmerov.
struct FormRef {
    id: ObjectId,
    w: f64,
    h: f64,
    /// Normalizácia zdrojovej strany do `[0,0,w,h]`.
    normalize: Matrix,
}

/// Zabalí zdrojovú stranu do Form XObjectu.
fn make_form(doc: &mut Document, page_id: ObjectId) -> Result<FormRef> {
    let (normalize, w, h) = page_geometry(doc, page_id);
    let content = doc.get_page_content(page_id);
    let resources = inherited(doc, page_id, b"Resources")
        .unwrap_or_else(|| Object::Dictionary(Dictionary::new()));
    let group = doc.get_dictionary(page_id).ok().and_then(|d| d.get(b"Group").ok()).cloned();

    // BBox musí byť vyjadrený v pôvodnom user space strany, nie v znormalizovanom.
    let media = media_rect(doc, page_id);
    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "FormType" => 1,
        "BBox" => vec![
            Object::Real(media.x as f32),
            Object::Real(media.y as f32),
            Object::Real((media.x + media.w) as f32),
            Object::Real((media.y + media.h) as f32),
        ],
        "Resources" => resources,
    };
    if let Some(group) = group {
        dict.set("Group", group);
    }
    let mut stream = Stream::new(dict, content);
    let _ = stream.compress();
    Ok(FormRef { id: doc.add_object(stream), w, h, normalize })
}

/// Vyskladá content stream jednej strany výstupu.
fn compose_side(
    side: &Side,
    forms: &BTreeMap<usize, FormRef>,
    sheet: (f64, f64),
    opts: &Options,
) -> (Vec<u8>, Dictionary) {
    let (sw, sh) = sheet;
    let margin = mm(opts.margin_mm);
    let gutter = mm(opts.gutter_mm);
    let slot_w = ((sw - 2.0 * margin - gutter) / 2.0).max(1.0);
    let slot_h = (sh - 2.0 * margin).max(1.0);

    // Creep: vonkajšie listy zošita posunieme k prehybu.
    let from_center = side.sheets_in_signature.saturating_sub(1 + side.sheet_in_signature);
    let creep = mm(opts.creep_mm) * from_center as f64;

    let slots = [
        Rect::new(margin + creep, margin, slot_w, slot_h),
        Rect::new(margin + slot_w + gutter - creep, margin, slot_w, slot_h),
    ];

    let mut out = String::new();
    let mut xobjects = Dictionary::new();
    for (i, slot) in side.slots.iter().enumerate() {
        let Some(page) = slot.page else { continue };
        let Some(form) = forms.get(&page) else { continue };
        let name = format!("X{i}");
        let m =
            form.normalize.then(geom::place(form.w, form.h, slots[i], opts.scale, slot.rotate180));
        out.push_str(&format!("q {} cm /{} Do Q\n", m.to_pdf(), name));
        xobjects.set(name, form.id);
    }
    out.push_str(&draw_marks(sheet, &slots, opts));
    (out.into_bytes(), xobjects)
}

fn draw_marks(sheet: (f64, f64), slots: &[Rect; 2], opts: &Options) -> String {
    if opts.marks == Marks::None {
        return String::new();
    }
    let (sw, sh) = sheet;
    let mut s = String::from("q 0.5 G 0.4 w\n");
    // Prehyb v strede listu.
    let fold = sw / 2.0;
    s.push_str(&format!("[4 4] 0 d {} 0 m {} {} l S\n", num(fold), num(fold), num(sh)));
    if opts.marks == Marks::FoldAndCrop {
        s.push_str("[] 0 d\n");
        let tick = mm(4.0);
        // Svislé značky na hornej a dolnej hrane v mieste svislých hrán slotov.
        for x in [slots[0].x, slots[0].x + slots[0].w, slots[1].x, slots[1].x + slots[1].w] {
            s.push_str(&format!("{} 0 m {} {} l S\n", num(x), num(x), num(tick)));
            s.push_str(&format!("{} {} m {} {} l S\n", num(x), num(sh - tick), num(x), num(sh)));
        }
        // Vodorovné značky na ľavej a pravej hrane.
        for y in [slots[0].y, slots[0].y + slots[0].h] {
            s.push_str(&format!("0 {} m {} {} l S\n", num(y), num(tick), num(y)));
            s.push_str(&format!("{} {} m {} {} l S\n", num(sw - tick), num(y), num(sw), num(y)));
        }
    }
    s.push_str("Q\n");
    s
}

/// Vráti normalizačnú maticu a efektívne rozmery strany.
fn page_geometry(doc: &Document, page_id: ObjectId) -> (Matrix, f64, f64) {
    let media = media_rect(doc, page_id);
    let rotate = inherited(doc, page_id, b"Rotate")
        .and_then(|o| doc.dereference(&o).ok().and_then(|(_, o)| o.as_i64().ok()))
        .unwrap_or(0);
    geom::normalize_page(media, rotate)
}

/// `/CropBox` obmedzený na `/MediaBox`, s dedením od `/Parent`.
fn media_rect(doc: &Document, page_id: ObjectId) -> Rect {
    let media =
        box_of(doc, page_id, b"MediaBox").unwrap_or(Rect::new(0.0, 0.0, mm(210.0), mm(297.0)));
    match box_of(doc, page_id, b"CropBox") {
        Some(crop) => intersect(media, crop).unwrap_or(media),
        None => media,
    }
}

fn box_of(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<Rect> {
    let obj = inherited(doc, page_id, key)?;
    let (_, obj) = doc.dereference(&obj).ok()?;
    let arr = obj.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let mut v = [0.0f64; 4];
    for (i, o) in arr.iter().take(4).enumerate() {
        let (_, o) = doc.dereference(o).ok()?;
        v[i] = o.as_float().ok()? as f64;
    }
    let (x0, x1) = (v[0].min(v[2]), v[0].max(v[2]));
    let (y0, y1) = (v[1].min(v[3]), v[1].max(v[3]));
    if x1 - x0 <= 0.0 || y1 - y0 <= 0.0 {
        return None;
    }
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    if x1 - x0 <= 0.0 || y1 - y0 <= 0.0 {
        None
    } else {
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

/// Nájde dediteľný atribút strany — najbližší `/Parent` vyhráva.
///
/// Vracia hodnotu *nedereferencovanú*, aby sa referencie na `/Resources` dali
/// použiť priamo a nemuseli sa kopírovať.
fn inherited(doc: &Document, page_id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut node = page_id;
    for _ in 0..64 {
        let dict = doc.get_dictionary(node).ok()?;
        if let Ok(value) = dict.get(key) {
            return Some(value.clone());
        }
        node = dict.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}
