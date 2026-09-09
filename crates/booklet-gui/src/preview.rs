//! Schématický náhľad impozície kreslený Cairom.
//!
//! Nezobrazuje skutočný obsah PDF, ale rozloženie strán na listoch —
//! to je presne to, čo treba pred tlačou skontrolovať.

use booklet_core::{mm, Face, FoldMark, Options, Plan, Rect, Side, Slot};
use gtk4::cairo::Context;

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
pub fn draw(
    cr: &Context,
    area_w: f64,
    area_h: f64,
    plan: &Plan,
    sheet: (f64, f64),
    opts: &Options,
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
        draw_side(cr, cell, side, i + 1, sheet, opts);
    }
}

fn draw_side(
    cr: &Context,
    cell: Rect,
    side: &Side,
    output_page: usize,
    sheet: (f64, f64),
    opts: &Options,
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
        draw_slot(cr, Rect::new(sx, top + margin, slot_w, slot_h), slot);
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

fn draw_slot(cr: &Context, rect: Rect, slot: &Slot) {
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
            cr.set_source_rgb(0.99, 0.99, 0.99);
            cr.rectangle(x, y, w, h);
            let _ = cr.fill_preserve();
            cr.set_source_rgb(0.72, 0.72, 0.72);
            cr.set_line_width(1.0);
            let _ = cr.stroke();

            let label = (page + 1).to_string();
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
                cr.set_source_rgb(0.55, 0.35, 0.1);
                cr.select_font_face(
                    "Sans",
                    gtk4::cairo::FontSlant::Normal,
                    gtk4::cairo::FontWeight::Normal,
                );
                cr.set_font_size(10.0);
                cr.move_to(x + 4.0, y + h - 5.0);
                let _ = cr.show_text("otočené 180°");
            }
        }
    }
}
