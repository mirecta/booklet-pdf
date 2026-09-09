//! Bitmapové náhľady zdrojových strán.
//!
//! Rasterizuje `hayro` (čistý Rust) na samostatnom vlákne, ktoré si drží
//! načítaný dokument. Náhľady sa počítajú až keď ich náhľad naozaj kreslí,
//! takže pri knihe s tisíckou strán sa vyrenderuje len to, čo je vidno.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::mpsc;

use gtk4::cairo::{Format, ImageSurface};
use gtk4::glib;

/// Zdroj náhľadov pre kreslenie — umožňuje náhľad bez rasterizéra.
pub trait ThumbSource {
    /// Náhľad strany `page` (0-based) široký aspoň `width_px` pixelov.
    ///
    /// Ak ešte nie je hotový, vráti `None` (alebo hrubší, už dostupný) a
    /// zaradí stranu na vyrenderovanie.
    fn thumb(&self, page: usize, width_px: i32) -> Option<ImageSurface>;
}

struct Request {
    page: usize,
    width: u16,
}

struct Rendered {
    page: usize,
    width: u16,
    w: i32,
    h: i32,
    /// Premultiplikované BGRA, riadok po riadku, so zarovnaním pre Cairo.
    data: Vec<u8>,
}

struct Cached {
    surface: ImageSurface,
    /// Šírka, na ktorú bola strana vyrenderovaná.
    width: u16,
}

/// Cache náhľadov nad jedným PDF.
pub struct Thumbnails {
    requests: mpsc::Sender<Request>,
    cache: RefCell<HashMap<usize, Cached>>,
    pending: RefCell<HashSet<usize>>,
    /// Strany, ktoré sa vyrenderovať nedali — nemá zmysel skúšať ich znova.
    failed: RefCell<HashSet<usize>>,
}

/// Šírky sa zaokrúhľujú na násobky 64 px, aby zmena veľkosti okna
/// nespôsobila prerenderovanie všetkého.
pub(crate) fn bucket(width_px: i32) -> u16 {
    let w = width_px.clamp(64, 768);
    ((w + 63) / 64 * 64) as u16
}

impl Thumbnails {
    /// Otvorí PDF na renderovanie náhľadov.
    ///
    /// `on_ready` sa zavolá na hlavnom vlákne vždy, keď dorazí nový náhľad.
    pub fn open(
        path: PathBuf,
        password: Option<String>,
        on_ready: impl Fn() + 'static,
    ) -> Rc<Self> {
        let (req_tx, req_rx) = mpsc::channel::<Request>();
        let (out_tx, out_rx) = async_channel::unbounded::<Rendered>();

        std::thread::Builder::new()
            .name("booklet-thumbs".into())
            .spawn(move || worker(path, password.unwrap_or_default(), req_rx, out_tx))
            .ok();

        let this = Rc::new(Self {
            requests: req_tx,
            cache: RefCell::new(HashMap::new()),
            pending: RefCell::new(HashSet::new()),
            failed: RefCell::new(HashSet::new()),
        });

        // Slabá referencia: keď sa otvorí iný súbor a `Thumbnails` zanikne,
        // slučka sa ukončí a s ňou aj renderovacie vlákno.
        let weak: Weak<Self> = Rc::downgrade(&this);
        glib::spawn_future_local(async move {
            while let Ok(done) = out_rx.recv().await {
                let Some(thumbs) = weak.upgrade() else { break };
                thumbs.pending.borrow_mut().remove(&done.page);
                match to_surface(&done) {
                    Some(surface) => {
                        thumbs
                            .cache
                            .borrow_mut()
                            .insert(done.page, Cached { surface, width: done.width });
                        on_ready();
                    }
                    None => {
                        thumbs.failed.borrow_mut().insert(done.page);
                    }
                }
            }
        });

        this
    }
}

impl ThumbSource for Thumbnails {
    fn thumb(&self, page: usize, width_px: i32) -> Option<ImageSurface> {
        if self.failed.borrow().contains(&page) {
            return None;
        }
        let want = bucket(width_px);
        let have = self.cache.borrow().get(&page).map(|c| (c.surface.clone(), c.width));
        match have {
            // Máme dosť veľký náhľad — hotovo.
            Some((surface, width)) if width >= want => Some(surface),
            // Máme len hrubší: ukáž ho a zároveň si vyžiadaj lepší.
            other => {
                if self.pending.borrow_mut().insert(page)
                    && self.requests.send(Request { page, width: want }).is_err()
                {
                    self.pending.borrow_mut().remove(&page);
                }
                other.map(|(surface, _)| surface)
            }
        }
    }
}

/// Prebalí BGRA dáta do Cairo povrchu. `None` znamená, že strana sa
/// vyrenderovať nedala.
fn to_surface(done: &Rendered) -> Option<ImageSurface> {
    if done.w <= 0 || done.h <= 0 || done.data.len() < done.w as usize * done.h as usize * 4 {
        return None;
    }
    let stride = Format::ARgb32.stride_for_width(done.w as u32).ok()?;
    let mut buffer = vec![0u8; stride as usize * done.h as usize];
    let row = done.w as usize * 4;
    for y in 0..done.h as usize {
        let src = y * row;
        let dst = y * stride as usize;
        buffer[dst..dst + row].copy_from_slice(&done.data[src..src + row]);
    }
    ImageSurface::create_for_data(buffer, Format::ARgb32, done.w, done.h, stride).ok()
}

/// Renderovacie vlákno — drží si načítaný dokument po celý čas.
fn worker(
    path: PathBuf,
    password: String,
    requests: mpsc::Receiver<Request>,
    out: async_channel::Sender<Rendered>,
) {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{RenderCache, RenderSettings};

    let Ok(data) = std::fs::read(&path) else { return };
    let Ok(pdf) = Pdf::new_with_password(data, &password) else { return };
    let cache = RenderCache::new();
    let interpret = InterpreterSettings::default();

    while let Ok(first) = requests.recv() {
        // Zober všetko, čo sa medzitým nazbieralo, a začni od najnovšieho —
        // to je najpravdepodobnejšie práve to, na čo sa používateľ díva.
        let mut batch = vec![first];
        while let Ok(next) = requests.try_recv() {
            batch.push(next);
        }
        for request in batch.into_iter().rev() {
            let pages = pdf.pages();
            let Some(page) = pages.get(request.page) else { continue };
            let (page_w, page_h) = page.render_dimensions();
            if page_w <= 0.0 || page_h <= 0.0 {
                continue;
            }
            let scale = request.width as f32 / page_w;
            let settings = RenderSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: WHITE,
                ..Default::default()
            };
            // PDF je nedôveryhodný vstup: keď rasterizér spadne, stránku
            // preskočíme a vlákno beží ďalej.
            let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let pixmap = hayro::render(page, &cache, &interpret, &settings);
                // hayro dáva premultiplikované RGBA, Cairo chce na
                // little-endian stroji BGRA.
                let mut data = pixmap.data_as_u8_slice().to_vec();
                for px in data.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                (pixmap.width() as i32, pixmap.height() as i32, data)
            }));

            let (w, h, data) = rendered.unwrap_or((0, 0, Vec::new()));
            let done = Rendered { page: request.page, width: request.width, w, h, data };
            if out.send_blocking(done).is_err() {
                return;
            }
        }
    }
}
