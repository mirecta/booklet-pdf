//! Náhľad impozície kreslený Cairom.
//!
//! Ukazuje rozloženie strán na listoch — a ak je k dispozícii rasterizér,
//! aj skutočný obsah strán.

use booklet_core::{mm, FoldMark, Lang, Options, Plan, Rect, Side, Slot};
use gtk4::cairo::{Context, Filter, ImageSurface};

use crate::thumbs::ThumbSource;

const GAP: f64 = 14.0;
const LABEL: f64 = 20.0;
const HEADING: f64 = 26.0;
const COLS: usize = 2;

/// Všetko, čo náhľad potrebuje vedieť.
pub struct Preview<'a> {
    pub plan: &'a Plan,
    /// Rozmer výstupného listu v bodoch.
    pub sheet: (f64, f64),
    pub opts: &'a Options,
    pub lang: Lang,
    /// Zdroj bitmapových náhľadov; bez neho sa kreslia len čísla strán.
    pub thumbs: Option<&'a dyn ThumbSource>,
    /// Násobok pixelov displeja.
    pub device_scale: f64,
}

/// Jeden prvok náhľadu — nadpis zošita alebo jedna strana výstupu.
pub enum Item {
    Heading { y: f64, text: String },
    Cell { rect: Rect, side: usize },
}

/// Rozloženie náhľadu vrátane celkovej výšky pre posuvník.
pub struct Layout {
    pub items: Vec<Item>,
    pub total_h: f64,
}

/// Rozvrhne strany výstupu do mriežky. Ak je zošitov viac, každý začína
/// novým riadkom a dostane nadpis — inak nie je z náhľadu vidno, že sa
/// dokument vôbec rozdelil.
pub fn layout(plan: &Plan, area_w: f64, sheet: (f64, f64), lang: Lang) -> Layout {
    let cell_w = ((area_w - GAP * (COLS as f64 + 1.0)) / COLS as f64).max(60.0);
    let ratio = if sheet.0 > 0.0 { sheet.1 / sheet.0 } else { 0.7 };
    let cell_h = cell_w * ratio + LABEL;

    let grouped = plan.is_grouped();
    let mut items = Vec::with_capacity(plan.sides.len() + plan.signatures.len());
    let mut y = GAP;
    let mut col = 0usize;
    let mut current = usize::MAX;

    for (i, side) in plan.sides.iter().enumerate() {
        if grouped && side.signature != current {
            if col != 0 {
                y += cell_h + GAP;
                col = 0;
            }
            current = side.signature;
            let sheets = plan.signatures.get(side.signature).copied().unwrap_or(0);
            items.push(Item::Heading {
                y,
                text: lang.signature_heading(side.signature + 1, plan.signatures.len(), sheets),
            });
            y += HEADING;
        }
        let x = GAP + col as f64 * (cell_w + GAP);
        items.push(Item::Cell { rect: Rect::new(x, y, cell_w, cell_h), side: i });
        col += 1;
        if col == COLS {
            col = 0;
            y += cell_h + GAP;
        }
    }
    if col != 0 {
        y += cell_h + GAP;
    }
    Layout { items, total_h: y + GAP }
}

/// Vykreslí celý náhľad. Prázdny plán zobrazí len výzvu na otvorenie PDF.
pub fn draw(cr: &Context, area_w: f64, area_h: f64, p: &Preview) {
    cr.set_source_rgb(0.96, 0.96, 0.95);
    let _ = cr.paint();

    if p.plan.sides.is_empty() {
        cr.set_source_rgb(0.45, 0.45, 0.45);
        sans(cr, false);
        cr.set_font_size(14.0);
        let text = p.lang.preview_empty();
        let ext = cr.text_extents(text).unwrap();
        cr.move_to((area_w - ext.width()) / 2.0, area_h / 2.0);
        let _ = cr.show_text(text);
        return;
    }

    for item in layout(p.plan, area_w, p.sheet, p.lang).items {
        match item {
            Item::Heading { y, text } => {
                cr.set_source_rgb(0.32, 0.32, 0.34);
                sans(cr, true);
                cr.set_font_size(13.0);
                cr.move_to(GAP, y + 16.0);
                let _ = cr.show_text(&text);
                cr.set_source_rgb(0.82, 0.82, 0.82);
                cr.set_line_width(1.0);
                cr.move_to(GAP, y + HEADING - 4.0);
                cr.line_to(area_w - GAP, y + HEADING - 4.0);
                let _ = cr.stroke();
            }
            Item::Cell { rect, side } => {
                let cell = Rect::new(rect.x, rect.y, rect.w, rect.h - LABEL);
                draw_side(cr, cell, &p.plan.sides[side], side + 1, p);
            }
        }
    }
}

fn sans(cr: &Context, bold: bool) {
    let weight = if bold { gtk4::cairo::FontWeight::Bold } else { gtk4::cairo::FontWeight::Normal };
    cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, weight);
}

fn draw_side(cr: &Context, cell: Rect, side: &Side, output_page: usize, p: &Preview) {
    let Rect { x, y, w, h } = cell;

    // Titulok nad listom. Pri viacerých zošitoch je dôležité, koľký list
    // zošita to je — podľa toho sa listy skladajú do seba.
    cr.set_source_rgb(0.25, 0.25, 0.25);
    sans(cr, true);
    cr.set_font_size(11.0);
    cr.move_to(x, y + 12.0);
    let in_signature = p.plan.is_grouped().then_some(side.sheet_in_signature + 1);
    let _ = cr.show_text(&p.lang.side_label(output_page, side.sheet + 1, side.face, in_signature));

    let top = y + LABEL;
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.rectangle(x, top, w, h);
    let _ = cr.fill_preserve();
    cr.set_source_rgb(0.62, 0.62, 0.62);
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    // Sloty v rovnakej geometrii ako vo výstupnom PDF.
    let scale = w / p.sheet.0.max(1.0);
    let margin = mm(p.opts.margin_mm) * scale;
    let gutter = mm(p.opts.gutter_mm) * scale;
    let slot_w = ((w - 2.0 * margin - gutter) / 2.0).max(4.0);
    let slot_h = (h - 2.0 * margin).max(4.0);

    for (i, slot) in side.slots.iter().enumerate() {
        let sx = x + margin + i as f64 * (slot_w + gutter);
        let thumb = slot
            .page
            .zip(p.thumbs)
            .and_then(|(page, src)| src.thumb(page, (slot_w * p.device_scale).round() as i32));
        draw_slot(cr, Rect::new(sx, top + margin, slot_w, slot_h), slot, thumb, p.lang);
    }

    draw_fold_hint(cr, x, top, w, h, p.opts.marks.fold);
}

fn draw_fold_hint(cr: &Context, x: f64, top: f64, w: f64, h: f64, fold: FoldMark) {
    let fold_x = x + w / 2.0;
    match fold {
        FoldMark::None => {}
        FoldMark::Line => {
            cr.set_source_rgb(0.55, 0.55, 0.55);
            cr.set_dash(&[3.0, 3.0], 0.0);
            cr.move_to(fold_x, top);
            cr.line_to(fold_x, top + h);
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
        }
        FoldMark::Ticks => {
            let tick = (h * 0.06).max(4.0);
            cr.set_source_rgb(0.35, 0.35, 0.35);
            cr.set_line_width(1.2);
            cr.move_to(fold_x, top);
            cr.line_to(fold_x, top + tick);
            cr.move_to(fold_x, top + h - tick);
            cr.line_to(fold_x, top + h);
            let _ = cr.stroke();
            cr.set_line_width(1.0);
        }
    }
}

fn draw_slot(cr: &Context, rect: Rect, slot: &Slot, thumb: Option<ImageSurface>, lang: Lang) {
    let Rect { x, y, w, h } = rect;
    match slot.page {
        None => draw_blank(cr, rect),
        Some(page) => {
            let label = (page + 1).to_string();
            cr.set_source_rgb(0.99, 0.99, 0.99);
            cr.rectangle(x, y, w, h);
            let _ = cr.fill();

            match thumb {
                // S náhľadom by veľké číslo cez obsah nebolo čitateľné ani
                // ono, ani strana — číslo ide do rohového odznaku.
                Some(surface) => {
                    paint_thumb(cr, rect, &surface, slot.rotate180);
                    stroke_border(cr, rect);
                    draw_badge(cr, rect, &label);
                }
                None => {
                    stroke_border(cr, rect);
                    cr.set_source_rgb(0.15, 0.15, 0.15);
                    sans(cr, true);
                    cr.set_font_size((h * 0.32).clamp(9.0, 46.0));
                    let ext = cr.text_extents(&label).unwrap();
                    let _ = cr.save();
                    cr.translate(x + w / 2.0, y + h / 2.0);
                    if slot.rotate180 {
                        cr.rotate(std::f64::consts::PI);
                    }
                    cr.move_to(-ext.width() / 2.0 - ext.x_bearing(), ext.height() / 2.0);
                    let _ = cr.show_text(&label);
                    let _ = cr.restore();
                }
            }
            if slot.rotate180 {
                draw_footnote(cr, rect, lang.rotated_note());
            }
        }
    }
}

/// Prázdne miesto: šrafovanie.
fn draw_blank(cr: &Context, rect: Rect) {
    let Rect { x, y, w, h } = rect;
    cr.set_source_rgb(0.88, 0.88, 0.88);
    cr.rectangle(x, y, w, h);
    let _ = cr.fill();
    cr.set_source_rgb(0.78, 0.78, 0.78);
    cr.set_line_width(1.0);
    let mut o = -h;
    while o < w {
        cr.move_to(x + o.max(0.0), y + (o.min(0.0)).abs());
        cr.line_to(x + (o + h).min(w), y + h - ((o + h) - w).max(0.0));
        o += 8.0;
    }
    let _ = cr.stroke();
}

fn stroke_border(cr: &Context, rect: Rect) {
    cr.rectangle(rect.x, rect.y, rect.w, rect.h);
    cr.set_source_rgb(0.72, 0.72, 0.72);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

/// Číslo strany ako odznak v hornom ľavom rohu slotu.
fn draw_badge(cr: &Context, rect: Rect, label: &str) {
    let size = (rect.h * 0.13).clamp(10.0, 20.0);
    sans(cr, true);
    cr.set_font_size(size);
    let Ok(ext) = cr.text_extents(label) else { return };
    let pad = size * 0.4;
    let (bw, bh) = (ext.width() + 2.0 * pad, size + pad);
    let (bx, by) = (rect.x + 5.0, rect.y + 5.0);

    rounded_rect(cr, Rect::new(bx, by, bw, bh), bh * 0.28);
    cr.set_source_rgba(0.11, 0.11, 0.12, 0.8);
    let _ = cr.fill();
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.move_to(bx + pad - ext.x_bearing(), by + bh - pad * 0.75);
    let _ = cr.show_text(label);
}

/// Poznámka pri dolnej hrane slotu, čitateľná aj nad obsahom strany.
fn draw_footnote(cr: &Context, rect: Rect, text: &str) {
    let size = (rect.h * 0.075).clamp(8.0, 12.0);
    sans(cr, false);
    cr.set_font_size(size);
    let Ok(ext) = cr.text_extents(text) else { return };
    let pad = size * 0.4;
    let bh = size + pad;
    cr.rectangle(rect.x + 1.0, rect.y + rect.h - bh - 1.0, ext.width() + 2.0 * pad, bh);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    let _ = cr.fill();
    cr.set_source_rgb(0.55, 0.35, 0.1);
    cr.move_to(rect.x + 1.0 + pad, rect.y + rect.h - 1.0 - pad * 0.75);
    let _ = cr.show_text(text);
}

fn rounded_rect(cr: &Context, rect: Rect, r: f64) {
    let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
    let hp = std::f64::consts::FRAC_PI_2;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -hp, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, hp);
    cr.arc(x + r, y + h - r, r, hp, 2.0 * hp);
    cr.arc(x + r, y + r, r, 2.0 * hp, 3.0 * hp);
    cr.close_path();
}

/// Vykreslí bitmapu strany do slotu so zachovaním pomeru strán.
fn paint_thumb(cr: &Context, rect: Rect, surface: &ImageSurface, rotate180: bool) {
    let (sw, sh) = (surface.width() as f64, surface.height() as f64);
    if sw <= 0.0 || sh <= 0.0 {
        return;
    }
    let scale = (rect.w / sw).min(rect.h / sh);
    let (pw, ph) = (sw * scale, sh * scale);
    let _ = cr.save();
    cr.rectangle(rect.x, rect.y, rect.w, rect.h);
    cr.clip();
    cr.translate(rect.x + (rect.w - pw) / 2.0, rect.y + (rect.h - ph) / 2.0);
    if rotate180 {
        cr.translate(pw, ph);
        cr.rotate(std::f64::consts::PI);
    }
    cr.scale(scale, scale);
    if cr.set_source_surface(surface, 0.0, 0.0).is_ok() {
        if let Ok(pattern) = cr.source().try_into() {
            let pattern: gtk4::cairo::SurfacePattern = pattern;
            pattern.set_filter(Filter::Good);
        }
        let _ = cr.paint();
    }
    let _ = cr.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use booklet_core::{Face, Marks, Side};
    use gtk4::cairo::Format;

    /// Náhľad, ktorý vždy vráti jednobarevný obrázok na mieste strany.
    struct FakeThumbs;

    impl ThumbSource for FakeThumbs {
        fn thumb(&self, _page: usize, width_px: i32) -> Option<ImageSurface> {
            let w = width_px.clamp(8, 200);
            let surface = ImageSurface::create(Format::ARgb32, w, w * 3 / 2).ok()?;
            let cr = Context::new(&surface).ok()?;
            cr.set_source_rgb(0.2, 0.4, 0.8);
            cr.paint().ok()?;
            drop(cr);
            Some(surface)
        }
    }

    fn side(face: Face, pages: [Option<usize>; 2], rotate180: bool) -> Side {
        Side {
            sheet: 0,
            signature: 0,
            sheet_in_signature: 0,
            sheets_in_signature: 1,
            face,
            slots: [Slot { page: pages[0], rotate180 }, Slot { page: pages[1], rotate180 }],
        }
    }

    fn sample_plan() -> Plan {
        Plan {
            sides: vec![
                side(Face::Front, [Some(3), Some(0)], false),
                // Rub pri obrate po dlhej hrane — otočené sloty.
                side(Face::Back, [Some(2), Some(1)], true),
                // Nepárny počet strán -> prázdne miesto.
                side(Face::Front, [None, Some(4)], false),
            ],
            sheets: 2,
            signatures: vec![2],
            source_pages: 5,
            blanks: 3,
        }
    }

    /// Plán s dvomi zošitmi — náhľad ich musí oddeliť nadpisom.
    fn grouped_plan() -> Plan {
        let mut sides = Vec::new();
        for signature in 0..2 {
            for face in [Face::Front, Face::Back] {
                sides.push(Side {
                    sheet: signature,
                    signature,
                    sheet_in_signature: 0,
                    sheets_in_signature: 1,
                    face,
                    slots: [Slot { page: Some(signature * 4), rotate180: false }; 2],
                });
            }
        }
        Plan { sides, sheets: 2, signatures: vec![1, 1], source_pages: 8, blanks: 0 }
    }

    fn render(
        plan: &Plan,
        thumbs: Option<&dyn ThumbSource>,
        opts: &Options,
        lang: Lang,
    ) -> ImageSurface {
        let surface = ImageSurface::create(Format::ARgb32, 700, 900).unwrap();
        let cr = Context::new(&surface).unwrap();
        let preview =
            Preview { plan, sheet: (841.89, 595.28), opts, lang, thumbs, device_scale: 1.0 };
        draw(&cr, 700.0, 900.0, &preview);
        cr.status().expect("cairo skončilo v chybovom stave");
        drop(cr);
        surface
    }

    /// Počet pixelov, ktoré sa líšia od farby pozadia náhľadu.
    fn non_background_pixels(mut surface: ImageSurface) -> usize {
        surface.flush();
        let data = surface.data().expect("povrch musí byť výhradne náš");
        let background = &data[0..4];
        data.chunks_exact(4).filter(|px| *px != background).count()
    }

    #[test]
    fn draws_without_thumbnails() {
        let painted =
            non_background_pixels(render(&sample_plan(), None, &Options::default(), Lang::En));
        assert!(painted > 1000);
    }

    #[test]
    fn draws_with_thumbnails_and_rotated_slots() {
        let painted = non_background_pixels(render(
            &sample_plan(),
            Some(&FakeThumbs),
            &Options::default(),
            Lang::Sk,
        ));
        assert!(painted > 10_000, "náhľady sa nevykreslili ({painted} pixelov)");
    }

    #[test]
    fn draws_empty_plan() {
        let empty =
            Plan { sides: vec![], sheets: 0, signatures: vec![], source_pages: 0, blanks: 0 };
        render(&empty, None, &Options::default(), Lang::Cs);
    }

    #[test]
    fn every_fold_mark_variant_draws() {
        for fold in [FoldMark::None, FoldMark::Ticks, FoldMark::Line] {
            for crop in [false, true] {
                let opts = Options {
                    marks: Marks { fold, crop },
                    margin_mm: 8.0,
                    gutter_mm: 5.0,
                    ..Options::default()
                };
                render(&sample_plan(), Some(&FakeThumbs), &opts, Lang::En);
            }
        }
    }

    #[test]
    fn draws_in_every_language() {
        for lang in Lang::ALL {
            render(&grouped_plan(), Some(&FakeThumbs), &Options::default(), *lang);
        }
    }

    #[test]
    fn signature_headings_add_height_and_start_a_new_row() {
        let sheet = (841.89, 595.28);
        let flat = layout(&grouped_plan(), 700.0, sheet, Lang::En);
        assert_eq!(flat.items.iter().filter(|i| matches!(i, Item::Heading { .. })).count(), 2);

        // Bez zoskupenia sa 4 strany vojdú do 2 riadkov, so zoskupením
        // potrebuje každý zošit vlastný riadok plus nadpis.
        let mut ungrouped = grouped_plan();
        ungrouped.signatures = vec![2];
        let plain = layout(&ungrouped, 700.0, sheet, Lang::En);
        assert!(plain.items.iter().all(|i| matches!(i, Item::Cell { .. })));
        assert!(flat.total_h > plain.total_h, "{} vs {}", flat.total_h, plain.total_h);
    }

    #[test]
    fn headings_follow_the_language() {
        let heading = |lang| match &layout(&grouped_plan(), 700.0, (841.89, 595.28), lang).items[0]
        {
            Item::Heading { text, .. } => text.clone(),
            _ => panic!("prvý prvok má byť nadpis"),
        };
        assert!(heading(Lang::En).starts_with("Signature 1 of 2"));
        assert!(heading(Lang::Sk).starts_with("Zošit 1 z 2"));
        assert!(heading(Lang::Cs).starts_with("Složka 1 z 2"));
    }

    #[test]
    fn buckets_round_up_and_clamp() {
        assert_eq!(crate::thumbs::bucket(1), 64);
        assert_eq!(crate::thumbs::bucket(65), 128);
        assert_eq!(crate::thumbs::bucket(128), 128);
        assert_eq!(crate::thumbs::bucket(10_000), 768);
    }
}
