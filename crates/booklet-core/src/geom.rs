//! Základná geometria: body/milimetre, transformačné matice, formáty papiera.

/// Prepočet milimetrov na PDF body (1 pt = 1/72").
pub const MM: f64 = 72.0 / 25.4;

/// Milimetre -> PDF body.
pub fn mm(value: f64) -> f64 {
    value * MM
}

/// PDF body -> milimetre.
pub fn to_mm(points: f64) -> f64 {
    points / MM
}

/// Obdĺžnik v PDF bodoch (ľavý dolný roh + rozmery).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

/// Afínna matica v PDF konvencii `[a b c d e f]`.
///
/// Bod sa transformuje ako riadkový vektor: `(x, y, 1) * M`, takže
/// „najprv A, potom B“ zodpovedá súčinu `A.then(B)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn translate(tx: f64, ty: f64) -> Self {
        Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: tx, f: ty }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Matrix { a: sx, b: 0.0, c: 0.0, d: sy, e: 0.0, f: 0.0 }
    }

    /// Rotácia proti smeru hodinových ručičiek o `deg` stupňov okolo počiatku.
    pub fn rotate(deg: f64) -> Self {
        let r = deg.to_radians();
        let (s, c) = (r.sin(), r.cos());
        Matrix { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }

    /// Zloženie: najprv `self`, potom `next`.
    pub fn then(self, next: Matrix) -> Self {
        Matrix {
            a: self.a * next.a + self.b * next.c,
            b: self.a * next.b + self.b * next.d,
            c: self.c * next.a + self.d * next.c,
            d: self.c * next.b + self.d * next.d,
            e: self.e * next.a + self.f * next.c + next.e,
            f: self.e * next.b + self.f * next.d + next.f,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    /// Zápis pre operátor `cm` v content streame.
    pub fn to_pdf(&self) -> String {
        format!(
            "{} {} {} {} {} {}",
            num(self.a),
            num(self.b),
            num(self.c),
            num(self.d),
            num(self.e),
            num(self.f)
        )
    }
}

/// Číslo bez exponentu a bez zbytočných nul — PDF neuznáva `1e-5`.
pub fn num(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v }; // zbav sa "-0"
    let mut s = format!("{:.5}", v);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s.is_empty() || s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Formát výstupného listu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paper {
    A6,
    A5,
    A4,
    A3,
    A2,
    Letter,
    Legal,
    Tabloid,
    /// Vlastný rozmer v milimetroch (šírka, výška v portrait orientácii).
    Custom {
        w_mm: f64,
        h_mm: f64,
    },
    /// Odvodiť z prvej strany zdroja tak, aby sa nemuselo nič zmenšovať.
    FromSource,
}

impl Paper {
    /// Rozmery v bodoch v portrait orientácii. `None` pre [`Paper::FromSource`].
    pub fn portrait_pt(&self) -> Option<(f64, f64)> {
        let (w, h) = match *self {
            Paper::A6 => (105.0, 148.0),
            Paper::A5 => (148.0, 210.0),
            Paper::A4 => (210.0, 297.0),
            Paper::A3 => (297.0, 420.0),
            Paper::A2 => (420.0, 594.0),
            Paper::Letter => (215.9, 279.4),
            Paper::Legal => (215.9, 355.6),
            Paper::Tabloid => (279.4, 431.8),
            Paper::Custom { w_mm, h_mm } => (w_mm, h_mm),
            Paper::FromSource => return None,
        };
        Some((mm(w), mm(h)))
    }

    /// Označenie formátu — nezávislé od jazyka. Pre [`Paper::Custom`] a
    /// [`Paper::FromSource`] použi [`crate::i18n::Lang::paper_label`].
    pub fn name(&self) -> &'static str {
        match *self {
            Paper::A6 => "A6",
            Paper::A5 => "A5",
            Paper::A4 => "A4",
            Paper::A3 => "A3",
            Paper::A2 => "A2",
            Paper::Letter => "Letter",
            Paper::Legal => "Legal",
            Paper::Tabloid => "Tabloid",
            Paper::Custom { .. } => "custom",
            Paper::FromSource => "source",
        }
    }

    pub const ALL: &'static [Paper] = &[
        Paper::A3,
        Paper::A4,
        Paper::A5,
        Paper::A6,
        Paper::A2,
        Paper::Letter,
        Paper::Legal,
        Paper::Tabloid,
        Paper::FromSource,
    ];
}

/// Orientácia výstupného listu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Šírka > výška — bežný prípad pre 2 strany vedľa seba.
    Landscape,
    Portrait,
}

impl Orientation {
    pub fn apply(&self, (w, h): (f64, f64)) -> (f64, f64) {
        let (long, short) = if w >= h { (w, h) } else { (h, w) };
        match self {
            Orientation::Landscape => (long, short),
            Orientation::Portrait => (short, long),
        }
    }
}

/// Normalizačná transformácia zdrojovej strany: z jej user space do
/// obdĺžnika `[0,0,w,h]`, so zapečeným `/Rotate`.
///
/// Vracia maticu a efektívne rozmery po rotácii.
pub fn normalize_page(media: Rect, rotate: i64) -> (Matrix, f64, f64) {
    let shift = Matrix::translate(-media.x, -media.y);
    let (w, h) = (media.w, media.h);
    // /Rotate je otočenie v smere hodinových ručičiek pri zobrazení.
    match rotate.rem_euclid(360) {
        90 => (shift.then(Matrix::rotate(-90.0)).then(Matrix::translate(0.0, w)), h, w),
        180 => (shift.then(Matrix::rotate(180.0)).then(Matrix::translate(w, h)), w, h),
        270 => (shift.then(Matrix::rotate(90.0)).then(Matrix::translate(h, 0.0)), h, w),
        _ => (shift, w, h),
    }
}

/// Umiestni obsah rozmeru `w × h` do rámca `slot`.
///
/// `scale` = false znamená mierku 1:1 (obsah sa len vycentruje, prípadne
/// preteká), `rotate180` otočí obsah v rámci slotu o 180°.
pub fn place(w: f64, h: f64, slot: Rect, scale: bool, rotate180: bool) -> Matrix {
    let s = if scale && w > 0.0 && h > 0.0 { (slot.w / w).min(slot.h / h) } else { 1.0 };
    let (pw, ph) = (w * s, h * s);
    let ox = slot.x + (slot.w - pw) / 2.0;
    let oy = slot.y + (slot.h - ph) / 2.0;
    let m = Matrix::scale(s, s).then(Matrix::translate(ox, oy));
    if rotate180 {
        // Otočenie o 180° okolo stredu umiestneného obsahu.
        let (cx, cy) = (ox + pw / 2.0, oy + ph / 2.0);
        Matrix::scale(s, s)
            .then(Matrix::translate(-pw / 2.0, -ph / 2.0))
            .then(Matrix::rotate(180.0))
            .then(Matrix::translate(cx, cy))
    } else {
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) {
        assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{a:?} != {b:?}");
    }

    #[test]
    fn normalize_rotate_90_maps_corners() {
        let media = Rect::new(0.0, 0.0, 200.0, 400.0);
        let (m, w, h) = normalize_page(media, 90);
        assert_eq!((w, h), (400.0, 200.0));
        close(m.apply(0.0, 0.0), (0.0, 200.0));
        close(m.apply(200.0, 400.0), (400.0, 0.0));
    }

    #[test]
    fn normalize_rotate_270_maps_corners() {
        let media = Rect::new(0.0, 0.0, 200.0, 400.0);
        let (m, w, h) = normalize_page(media, 270);
        assert_eq!((w, h), (400.0, 200.0));
        close(m.apply(0.0, 0.0), (400.0, 0.0));
        close(m.apply(200.0, 400.0), (0.0, 200.0));
    }

    #[test]
    fn normalize_offset_mediabox() {
        let media = Rect::new(10.0, 20.0, 100.0, 200.0);
        let (m, w, h) = normalize_page(media, 0);
        assert_eq!((w, h), (100.0, 200.0));
        close(m.apply(10.0, 20.0), (0.0, 0.0));
    }

    #[test]
    fn place_fits_and_centers() {
        let slot = Rect::new(0.0, 0.0, 100.0, 100.0);
        let m = place(50.0, 200.0, slot, true, false);
        // mierka 0.5 -> 25 x 100, vycentrované vodorovne
        close(m.apply(0.0, 0.0), (37.5, 0.0));
        close(m.apply(50.0, 200.0), (62.5, 100.0));
    }

    #[test]
    fn place_rotated_180_swaps_corners() {
        let slot = Rect::new(0.0, 0.0, 100.0, 200.0);
        let m = place(100.0, 200.0, slot, true, true);
        close(m.apply(0.0, 0.0), (100.0, 200.0));
        close(m.apply(100.0, 200.0), (0.0, 0.0));
    }

    #[test]
    fn num_avoids_exponent() {
        assert_eq!(num(0.000001), "0");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(300.0), "300");
    }
}
