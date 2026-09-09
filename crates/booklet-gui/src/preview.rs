//! Schématický náhľad impozície kreslený Cairom.
//!
//! Nezobrazuje skutočný obsah PDF, ale rozloženie strán na listoch —
//! to je presne to, čo treba pred tlačou skontrolovať.

use booklet_core::{mm, Face, FoldMark, Options, Plan, Rect, Side, Slot};
use gtk4::cairo::{Context, Filter, ImageSurface};

use crate::thumbs::ThumbSource;

const GAP: f64 = 14.0;
const LABEL: f64 = 20.0;
const COLS: usize = 2;

/// Rozmery mriežky náhľadu.
pub struct Layout {
    pub cell_w: f64,
    pub cell_h: f64,
    pub total_h: f64,
}

pub fn layout(sides: usize, area_w: f64, sheet: (f64, f64)) -> Layout {
    let cell_w = ((area_w - GAP * (COLS as f64 + 1.0)) / COLS as f64).max(60.0);
    let ratio = if sheet.0 > 0.0 { sheet.1 / sheet.0 } else { 0.7 };
    let cell_h = cell_w * ratio + LABEL;
    let rows = sides.div_ceil(COLS);
    Layout { cell_w, cell_h, total_h: rows as f64 * (cell_h + GAP) + GAP }
}

/// Vykreslí celý náhľad. `plan` môže byť prázdny.
///
/// `thumbs` je nepovinný zdroj bitmapových náhľadov strán; bez neho sa
/// kreslia len čísla strán. `device_scale` je násobok pixelov displeja.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    cr: &Context,
    area_w: f64,
    area_h: f64,
    plan: &Plan,
    sheet: (f64, f64),
    opts: &Options,
    thumbs: Option<&dyn ThumbSource>,
    device_scale: f64,
) {
    cr.set_source_rgb(0.96, 0.96, 0.95);
    let _ = cr.paint();

    if plan.sides.is_empty() {
        cr.set_source_rgb(0.45, 0.45, 0.45);
        cr.select_font_face(
            "Sans",
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Normal,
        );
        cr.set_font_size(14.0);
        let text = "Otvor PDF a tu sa zobrazí rozloženie strán na listoch.";
        let ext = cr.text_extents(text).unwrap();
        cr.move_to((area_w - ext.width()) / 2.0, area_h / 2.0);
        let _ = cr.show_text(text);
        return;
    }

    let l = layout(plan.sides.len(), area_w, sheet);
    for (i, side) in plan.sides.iter().enumerate() {
        let col = i % COLS;
        let row = i / COLS;
        let x = GAP + col as f64 * (l.cell_w + GAP);
        let y = GAP + row as f64 * (l.cell_h + GAP);
        let cell = Rect::new(x, y, l.cell_w, l.cell_h - LABEL);
        draw_side(cr, cell, side, i + 1, sheet, opts, thumbs, device_scale);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_side(
    cr: &Context,
    cell: Rect,
    side: &Side,
    output_page: usize,
    sheet: (f64, f64),
    opts: &Options,
    thumbs: Option<&dyn ThumbSource>,
    device_scale: f64,
) {
    let Rect { x, y, w, h } = cell;
    // Titulok nad listom.
    cr.set_source_rgb(0.25, 0.25, 0.25);
    cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Bold);
    cr.set_font_size(11.0);
    cr.move_to(x, y + 12.0);
    let face = match side.face {
        Face::Front => "líce",
        Face::Back => "rub",
    };
    let sig =
        if side.signature > 0 { format!(", zošit {}", side.signature + 1) } else { String::new() };
    let _ = cr
        .show_text(&format!("{output_page}. strana výstupu — list {} {face}{sig}", side.sheet + 1));

    let top = y + LABEL;
    // Papier.
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.rectangle(x, top, w, h);
    let _ = cr.fill_preserve();
    cr.set_source_rgb(0.62, 0.62, 0.62);
    cr.set_line_width(1.0);
    let _ = cr.stroke();

    // Sloty v rovnakej geometrii ako vo výstupnom PDF.
    let scale = w / sheet.0.max(1.0);
    let margin = mm(opts.margin_mm) * scale;
    let gutter = mm(opts.gutter_mm) * scale;
    let slot_w = ((w - 2.0 * margin - gutter) / 2.0).max(4.0);
    let slot_h = (h - 2.0 * margin).max(4.0);

    for (i, slot) in side.slots.iter().enumerate() {
        let sx = x + margin + i as f64 * (slot_w + gutter);
        let thumb = slot
            .page
            .zip(thumbs)
            .and_then(|(page, src)| src.thumb(page, (slot_w * device_scale).round() as i32));
        draw_slot(cr, Rect::new(sx, top + margin, slot_w, slot_h), slot, thumb);
    }

    let fold_x = x + w / 2.0;
    match opts.marks.fold {
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

fn stroke_border(cr: &Context, rect: Rect) {
    cr.rectangle(rect.x, rect.y, rect.w, rect.h);
    cr.set_source_rgb(0.72, 0.72, 0.72);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

/// Číslo strany ako odznak v hornom ľavom rohu slotu.
fn draw_badge(cr: &Context, rect: Rect, label: &str) {
    let size = (rect.h * 0.13).clamp(10.0, 20.0);
    cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Bold);
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
    cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Normal);
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

fn draw_slot(cr: &Context, rect: Rect, slot: &Slot, thumb: Option<ImageSurface>) {
    let Rect { x, y, w, h } = rect;
    match slot.page {
        None => {
            // Prázdne miesto: šrafovanie.
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
                    if slot.rotate180 {
                        draw_footnote(cr, rect, "otočené 180°");
                    }
                }
                None => {
                    stroke_border(cr, rect);
                    cr.set_source_rgb(0.15, 0.15, 0.15);
                    cr.select_font_face(
                        "Sans",
                        gtk4::cairo::FontSlant::Normal,
                        gtk4::cairo::FontWeight::Bold,
                    );
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
                    if slot.rotate180 {
                        draw_footnote(cr, rect, "otočené 180°");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use booklet_core::{Face, Marks, PlanOptions, Side};
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
            source_pages: 5,
            blanks: 3,
        }
    }

    /// Vykreslí náhľad a vráti, či sa v Cairo kontexte nič nerozbilo.
    fn render(thumbs: Option<&dyn ThumbSource>, opts: &Options) -> ImageSurface {
        let surface = ImageSurface::create(Format::ARgb32, 700, 900).unwrap();
        let cr = Context::new(&surface).unwrap();
        draw(&cr, 700.0, 900.0, &sample_plan(), (841.89, 595.28), opts, thumbs, 1.0);
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
        assert!(non_background_pixels(render(None, &Options::default())) > 1000);
    }

    #[test]
    fn draws_with_thumbnails_and_rotated_slots() {
        let opts = Options { marks: Marks::default(), ..Options::default() };
        let painted = non_background_pixels(render(Some(&FakeThumbs), &opts));
        assert!(painted > 10_000, "náhľady sa nevykreslili ({painted} pixelov)");
    }

    #[test]
    fn draws_empty_plan() {
        let surface = ImageSurface::create(Format::ARgb32, 400, 200).unwrap();
        let cr = Context::new(&surface).unwrap();
        let empty = Plan { sides: vec![], sheets: 0, source_pages: 0, blanks: 0 };
        draw(&cr, 400.0, 200.0, &empty, (841.89, 595.28), &Options::default(), None, 1.0);
        cr.status().expect("cairo skončilo v chybovom stave");
    }

    #[test]
    fn every_fold_mark_variant_draws() {
        for fold in [FoldMark::None, FoldMark::Ticks, FoldMark::Line] {
            for crop in [false, true] {
                let opts = Options {
                    marks: Marks { fold, crop },
                    margin_mm: 8.0,
                    gutter_mm: 5.0,
                    plan: PlanOptions::default(),
                    ..Options::default()
                };
                render(Some(&FakeThumbs), &opts);
            }
        }
    }

    #[test]
    fn buckets_round_up_and_clamp() {
        assert_eq!(crate::thumbs::bucket(1), 64);
        assert_eq!(crate::thumbs::bucket(65), 128);
        assert_eq!(crate::thumbs::bucket(128), 128);
        assert_eq!(crate::thumbs::bucket(10_000), 768);
    }
}
