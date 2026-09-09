//! GTK4 aplikácia na prepočet PDF do brožúry / zošitov.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod preview;
mod thumbs;

use crate::thumbs::{ThumbSource, Thumbnails};

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use booklet_core::{
    impose_file, info, plan as planner, Binding, Error, Flip, FoldMark, Marks, Mode, Options,
    Orientation as SheetOrientation, Paper, PdfInfo, Plan, PlanOptions, SheetOrder,
};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    AlertDialog, Application, ApplicationWindow, Box as GBox, Button, CheckButton, DrawingArea,
    DropDown, Entry, FileDialog, FileFilter, Frame, Grid, HeaderBar, Label, Orientation,
    ScrolledWindow, SpinButton,
};

const APP_ID: &str = "sk.booklet.BookletPdf";

const MODES: &[&str] = &[
    "Brožúra – zošitá v strede",
    "Zošity – skladané po častiach",
    "2 strany na list – bez skladania",
];
const ORIENTATIONS: &[&str] = &["Na ležato", "Na stojato"];
const BINDINGS: &[&str] = &["Vľavo (bežné)", "Vpravo (RTL, manga)"];
const FLIPS: &[&str] = &["po krátkej hrane", "po dlhej hrane"];
const ORDERS: &[&str] =
    &["Duplex – líce/rub za sebou", "Ručný duplex – najprv líca", "Ručný duplex – ruby odzadu"];
const FOLDS: &[&str] = &["Neznačiť", "Značky pri hranách listu", "Prerušovaná čiara cez list"];

/// Widgety, ktoré treba čítať pri každej zmene.
struct Ui {
    window: ApplicationWindow,
    file_label: Label,
    status: Label,
    mode: DropDown,
    sheets: SpinButton,
    sheets_row: GBox,
    paper: DropDown,
    orientation: DropDown,
    binding: DropDown,
    flip: DropDown,
    order: DropDown,
    margin: SpinButton,
    gutter: SpinButton,
    creep: SpinButton,
    scale: CheckButton,
    show_thumbs: CheckButton,
    fold: DropDown,
    crop: CheckButton,
    range: Entry,
    save_button: Button,
    area: DrawingArea,
    state: RefCell<State>,
}

#[derive(Default)]
struct State {
    path: Option<PathBuf>,
    info: Option<PdfInfo>,
    password: Option<String>,
    /// Aktuálny plán a rozmer listu pre náhľad.
    plan: Option<Plan>,
    sheet: (f64, f64),
    busy: bool,
    thumbs: Option<Rc<Thumbnails>>,
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_activate(|app| {
        build(app);
    });
    // `booklet-gui subor.pdf` aj „Otvoriť s…“ zo správcu súborov.
    app.connect_open(|app, files, _| {
        let ui = build(app);
        if let Some(path) = files.first().and_then(|f| f.path()) {
            load_input(&ui, path);
        }
    });
    app.run()
}

fn build(app: &Application) -> Rc<Ui> {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Booklet PDF")
        .default_width(1120)
        .default_height(780)
        .build();

    let open_button = Button::from_icon_name("document-open-symbolic");
    open_button.set_tooltip_text(Some("Otvoriť PDF (Ctrl+O)"));
    let save_button = Button::with_label("Uložiť PDF…");
    save_button.add_css_class("suggested-action");
    save_button.set_sensitive(false);

    let header = HeaderBar::new();
    header.pack_start(&open_button);
    header.pack_end(&save_button);
    window.set_titlebar(Some(&header));

    let area = DrawingArea::new();
    area.set_content_height(400);

    let ui = Rc::new(Ui {
        window: window.clone(),
        file_label: Label::builder()
            .label("Žiadny súbor")
            .wrap(true)
            .xalign(0.0)
            .max_width_chars(34)
            .build(),
        status: Label::builder()
            .label("Otvor PDF.")
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(30)
            .build(),
        mode: DropDown::from_strings(MODES),
        sheets: SpinButton::with_range(1.0, 60.0, 1.0),
        sheets_row: GBox::new(Orientation::Horizontal, 8),
        paper: DropDown::from_strings(
            &Paper::ALL
                .iter()
                .map(|p| p.label())
                .collect::<Vec<_>>()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        ),
        orientation: DropDown::from_strings(ORIENTATIONS),
        binding: DropDown::from_strings(BINDINGS),
        flip: DropDown::from_strings(FLIPS),
        order: DropDown::from_strings(ORDERS),
        margin: SpinButton::with_range(0.0, 50.0, 0.5),
        gutter: SpinButton::with_range(0.0, 60.0, 0.5),
        creep: SpinButton::with_range(0.0, 2.0, 0.05),
        scale: CheckButton::with_label("Prispôsobiť mierku na list"),
        show_thumbs: CheckButton::with_label("Náhľady strán"),
        fold: DropDown::from_strings(FOLDS),
        crop: CheckButton::with_label("Orezové značky na hranách"),
        range: Entry::builder().placeholder_text("všetky, napr. 1-8,11").build(),
        save_button: save_button.clone(),
        area: area.clone(),
        state: RefCell::new(State { sheet: (0.0, 0.0), ..State::default() }),
    });

    ui.mode.set_selected(0);
    ui.paper.set_selected(Paper::ALL.iter().position(|p| *p == Paper::A4).unwrap_or(0) as u32);
    ui.sheets.set_value(4.0);
    ui.creep.set_digits(2);
    ui.margin.set_digits(1);
    ui.gutter.set_digits(1);
    ui.scale.set_active(true);
    ui.show_thumbs.set_active(true);
    ui.show_thumbs.set_tooltip_text(Some(
        "Vykreslí skutočný obsah strán pod čísla. Renderuje sa na pozadí\n\
         a len to, čo je práve vidno.",
    ));
    ui.scale.set_tooltip_text(Some("Vypnuté = mierka 1:1, obsah sa môže nezmestiť."));
    ui.sheets.set_hexpand(true);
    ui.fold.set_selected(1);
    ui.fold.set_tooltip_text(Some(
        "Značky pri hranách ukážu, kde list prehnúť, a nekreslia sa cez obsah strán.\n\
         Prerušovaná čiara je viditeľnejšia, ale zostane vytlačená v knižke.",
    ));

    let content = GBox::new(Orientation::Horizontal, 0);
    content.append(&sidebar(&ui));
    content.append(&gtk4::Separator::new(Orientation::Vertical));

    let scroller = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .hexpand(true)
        .vexpand(true)
        .child(&area)
        .build();
    content.append(&scroller);
    window.set_child(Some(&content));

    // Kreslenie náhľadu.
    {
        let ui = ui.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let (plan, sheet) = {
                let state = ui.state.borrow();
                let plan = state.plan.clone().unwrap_or(Plan {
                    sides: vec![],
                    sheets: 0,
                    source_pages: 0,
                    blanks: 0,
                });
                let sheet = if state.sheet.0 > 0.0 { state.sheet } else { (841.89, 595.28) };
                (plan, sheet)
            };
            let thumbs =
                if ui.show_thumbs.is_active() { ui.state.borrow().thumbs.clone() } else { None };
            preview::draw(
                cr,
                w as f64,
                h as f64,
                &plan,
                sheet,
                &read_options(&ui),
                thumbs.as_deref().map(|t| t as &dyn ThumbSource),
                ui.area.scale_factor().max(1) as f64,
            );
        });
    }
    {
        let ui = ui.clone();
        area.connect_resize(move |_, _, _| resize_preview(&ui));
    }

    // Prepojenie ovládacích prvkov.
    for dd in [&ui.mode, &ui.paper, &ui.orientation, &ui.binding, &ui.flip, &ui.order, &ui.fold] {
        let ui = ui.clone();
        dd.connect_selected_notify(move |_| refresh(&ui));
    }
    for sb in [&ui.sheets, &ui.margin, &ui.gutter, &ui.creep] {
        let ui = ui.clone();
        sb.connect_value_changed(move |_| refresh(&ui));
    }
    for check in [&ui.scale, &ui.crop, &ui.show_thumbs] {
        let handler = ui.clone();
        check.connect_toggled(move |_| refresh(&handler));
    }
    {
        let handler = ui.clone();
        ui.range.connect_changed(move |_| refresh(&handler));
    }
    {
        let ui = ui.clone();
        open_button.connect_clicked(move |_| choose_input(&ui));
    }
    {
        let ui = ui.clone();
        save_button.connect_clicked(move |_| choose_output(&ui));
    }

    // Ctrl+O / Ctrl+S.
    let keys = gtk4::ShortcutController::new();
    keys.set_scope(gtk4::ShortcutScope::Global);
    let ui_open = ui.clone();
    keys.add_shortcut(gtk4::Shortcut::new(
        gtk4::ShortcutTrigger::parse_string("<Control>o"),
        Some(gtk4::CallbackAction::new(move |_, _| {
            choose_input(&ui_open);
            glib::Propagation::Stop
        })),
    ));
    let ui_save = ui.clone();
    keys.add_shortcut(gtk4::Shortcut::new(
        gtk4::ShortcutTrigger::parse_string("<Control>s"),
        Some(gtk4::CallbackAction::new(move |_, _| {
            if ui_save.save_button.is_sensitive() {
                choose_output(&ui_save);
            }
            glib::Propagation::Stop
        })),
    ));
    window.add_controller(keys);

    enable_drop(&ui);
    sync_sensitivity(&ui);
    window.present();
    ui
}

fn sidebar(ui: &Rc<Ui>) -> ScrolledWindow {
    let outer = GBox::new(Orientation::Vertical, 12);
    outer.set_margin_top(14);
    outer.set_margin_bottom(14);
    outer.set_margin_start(14);
    outer.set_margin_end(14);

    outer.append(&section("Vstup"));
    let file_box = GBox::new(Orientation::Vertical, 4);
    file_box.append(&ui.file_label);
    outer.append(&framed(&file_box));

    outer.append(&section("Skladanie"));
    let grid = new_grid();
    let mut r = 0;
    add_row(&grid, &mut r, "Režim", &ui.mode);
    ui.sheets_row.append(&ui.sheets);
    add_row(&grid, &mut r, "Listov v zošite", &ui.sheets_row);
    add_row(&grid, &mut r, "Väzba", &ui.binding);
    add_row(&grid, &mut r, "Strany", &ui.range);
    outer.append(&framed(&grid));

    outer.append(&section("List papiera"));
    let grid = new_grid();
    let mut r = 0;
    add_row(&grid, &mut r, "Formát", &ui.paper);
    add_row(&grid, &mut r, "Orientácia", &ui.orientation);
    add_row(&grid, &mut r, "Okraj (mm)", &ui.margin);
    add_row(&grid, &mut r, "Prehyb (mm)", &ui.gutter);
    add_row(&grid, &mut r, "Prehyb značiť", &ui.fold);
    grid.attach(&ui.crop, 0, r, 2, 1);
    r += 1;
    grid.attach(&ui.scale, 0, r, 2, 1);
    r += 1;
    grid.attach(&ui.show_thumbs, 0, r, 2, 1);
    outer.append(&framed(&grid));

    outer.append(&section("Tlač"));
    let grid = new_grid();
    let mut r = 0;
    add_row(&grid, &mut r, "Obrat papiera", &ui.flip);
    add_row(&grid, &mut r, "Poradie", &ui.order);
    add_row(&grid, &mut r, "Creep (mm/list)", &ui.creep);
    ui.flip.set_tooltip_text(Some(
        "Musí sedieť s nastavením duplexu v ovládači tlačiarne.\n\
         Ak je rub hlavou dolu, prepni túto voľbu.",
    ));
    ui.creep.set_tooltip_text(Some(
        "Posunie obsah vonkajších listov k prehybu, aby po orezaní\n\
         vyšli okraje rovnako. Pri tenkých brožúrach nechaj 0.",
    ));
    outer.append(&framed(&grid));

    ui.status.add_css_class("dim-label");
    outer.append(&ui.status);

    ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .hexpand(false)
        .width_request(390)
        .propagate_natural_width(false)
        .child(&outer)
        .build()
}

fn new_grid() -> Grid {
    let grid = Grid::new();
    grid.set_row_spacing(6);
    grid.set_column_spacing(10);
    grid.set_margin_top(8);
    grid.set_margin_bottom(8);
    grid.set_margin_start(8);
    grid.set_margin_end(8);
    grid
}

fn add_row(grid: &Grid, row: &mut i32, label: &str, widget: &impl IsA<gtk4::Widget>) {
    let l = Label::builder().label(label).xalign(0.0).build();
    l.add_css_class("dim-label");
    grid.attach(&l, 0, *row, 1, 1);
    let w = widget.as_ref();
    w.set_hexpand(true);
    grid.attach(w, 1, *row, 1, 1);
    *row += 1;
}

fn section(title: &str) -> Label {
    let l = Label::builder().use_markup(true).xalign(0.0).build();
    l.set_markup(&format!("<b>{title}</b>"));
    l
}

fn framed(child: &impl IsA<gtk4::Widget>) -> Frame {
    Frame::builder().child(child).build()
}

/// Prečíta nastavenia z widgetov.
fn read_options(ui: &Rc<Ui>) -> Options {
    Options {
        plan: PlanOptions {
            mode: match ui.mode.selected() {
                1 => Mode::Signatures { sheets: ui.sheets.value().round().max(1.0) as usize },
                2 => Mode::TwoUp,
                _ => Mode::Booklet,
            },
            binding: if ui.binding.selected() == 1 { Binding::Right } else { Binding::Left },
            flip: if ui.flip.selected() == 1 { Flip::LongEdge } else { Flip::ShortEdge },
            order: match ui.order.selected() {
                1 => SheetOrder::FrontsThenBacks,
                2 => SheetOrder::FrontsThenBacksReversed,
                _ => SheetOrder::Interleaved,
            },
        },
        paper: Paper::ALL.get(ui.paper.selected() as usize).copied().unwrap_or(Paper::A4),
        orientation: if ui.orientation.selected() == 1 {
            SheetOrientation::Portrait
        } else {
            SheetOrientation::Landscape
        },
        margin_mm: ui.margin.value(),
        gutter_mm: ui.gutter.value(),
        creep_mm: ui.creep.value(),
        scale: ui.scale.is_active(),
        marks: Marks {
            fold: match ui.fold.selected() {
                0 => FoldMark::None,
                2 => FoldMark::Line,
                _ => FoldMark::Ticks,
            },
            crop: ui.crop.is_active(),
        },
        range: ui.range.text().to_string(),
        password: ui.state.borrow().password.clone(),
    }
}

fn sync_sensitivity(ui: &Rc<Ui>) {
    let signatures = ui.mode.selected() == 1;
    ui.sheets_row.set_sensitive(signatures);
    ui.creep.set_sensitive(ui.mode.selected() != 2);
}

/// Prepočíta plán a náhľad podľa aktuálnych nastavení.
fn refresh(ui: &Rc<Ui>) {
    sync_sensitivity(ui);
    let opts = read_options(ui);
    let loaded = ui.state.borrow().info.as_ref().map(|i| (i.pages, i.first_page_pt));
    let Some((pages, first)) = loaded else {
        ui.state.borrow_mut().plan = None;
        ui.area.queue_draw();
        return;
    };

    let selection = match planner::parse_range(&opts.range, pages) {
        Ok(s) => s,
        Err(e) => {
            ui.status.set_markup(&format!("<span foreground='#c01c28'>{}</span>", esc(&e)));
            ui.save_button.set_sensitive(false);
            ui.state.borrow_mut().plan = None;
            ui.area.queue_draw();
            return;
        }
    };
    let plan = planner::plan(&selection, &opts.plan);
    let sheet = match opts.paper.portrait_pt() {
        Some(p) => opts.orientation.apply(p),
        None => (
            2.0 * first.0
                + 2.0 * booklet_core::mm(opts.margin_mm)
                + booklet_core::mm(opts.gutter_mm),
            first.1 + 2.0 * booklet_core::mm(opts.margin_mm),
        ),
    };

    let busy = {
        let mut state = ui.state.borrow_mut();
        state.plan = Some(plan.clone());
        state.sheet = sheet;
        state.busy
    };
    ui.save_button.set_sensitive(!busy && !plan.sides.is_empty());
    ui.status.set_markup(&esc(&format!(
        "{} strán zdroja → {} listov papiera ({} strán výstupu), {} prázdnych miest.\nList {:.0}×{:.0} mm.",
        plan.source_pages,
        plan.sheets,
        plan.output_pages(),
        plan.blanks,
        booklet_core::to_mm(sheet.0),
        booklet_core::to_mm(sheet.1),
    )));
    resize_preview(ui);
    ui.area.queue_draw();
}

fn resize_preview(ui: &Rc<Ui>) {
    let state = ui.state.borrow();
    let sides = state.plan.as_ref().map_or(0, |p| p.sides.len());
    let sheet = if state.sheet.0 > 0.0 { state.sheet } else { (841.89, 595.28) };
    drop(state);
    let w = ui.area.width().max(1);
    let h = if sides == 0 { 400.0 } else { preview::layout(sides, w as f64, sheet).total_h };
    ui.area.set_content_height(h.round() as i32);
}

fn pdf_filter() -> gio::ListStore {
    let filter = FileFilter::new();
    filter.set_name(Some("PDF"));
    filter.add_mime_type("application/pdf");
    filter.add_suffix("pdf");
    let store = gio::ListStore::new::<FileFilter>();
    store.append(&filter);
    store
}

fn choose_input(ui: &Rc<Ui>) {
    let dialog =
        FileDialog::builder().title("Otvoriť PDF").filters(&pdf_filter()).modal(true).build();
    let ui = ui.clone();
    dialog.open(Some(&ui.window.clone()), gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                load_input(&ui, path);
            }
        }
    });
}

fn load_input(ui: &Rc<Ui>, path: PathBuf) {
    // Heslo si vytiahni skôr — `borrow()` v hlavičke `match` by žil až do
    // konca vetvy a kolidoval s `borrow_mut()`.
    let password = ui.state.borrow().password.clone();
    match info(&path, password.as_deref()) {
        Ok(pdf) => {
            let name =
                path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let (w, h) = pdf.first_page_pt;
            ui.file_label.set_markup(&esc(&format!(
                "{name}\n{} strán, prvá strana {:.0}×{:.0} mm",
                pdf.pages,
                booklet_core::to_mm(w),
                booklet_core::to_mm(h),
            )));
            let redraw = {
                let area = ui.area.clone();
                move || area.queue_draw()
            };
            let thumbs = Thumbnails::open(path.clone(), password.clone(), redraw);
            let mut state = ui.state.borrow_mut();
            state.path = Some(path);
            state.info = Some(pdf);
            // Starý renderer zaniká spolu s posledným Rc a vlákno sa ukončí.
            state.thumbs = Some(thumbs);
            drop(state);
            refresh(ui);
        }
        Err(Error::Encrypted) => ask_password(ui, path),
        Err(e) => {
            error_dialog(ui, &format!("{e}"));
        }
    }
}

/// Modálny dotaz na heslo zašifrovaného PDF.
fn ask_password(ui: &Rc<Ui>, path: PathBuf) {
    let entry = gtk4::PasswordEntry::builder().show_peek_icon(true).build();
    let content = GBox::new(Orientation::Vertical, 10);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.append(&Label::new(Some("PDF je chránené heslom.")));
    content.append(&entry);
    let buttons = GBox::new(Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    let cancel = Button::with_label("Zrušiť");
    let ok = Button::with_label("Otvoriť");
    ok.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&ok);
    content.append(&buttons);

    let dialog = gtk4::Window::builder()
        .title("Heslo")
        .transient_for(&ui.window)
        .modal(true)
        .resizable(false)
        .child(&content)
        .build();

    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }
    let submit = {
        let ui = ui.clone();
        let dialog = dialog.clone();
        let entry = entry.clone();
        move || {
            ui.state.borrow_mut().password = Some(entry.text().to_string());
            dialog.close();
            load_input(&ui, path.clone());
        }
    };
    {
        let submit = submit.clone();
        ok.connect_clicked(move |_| submit());
    }
    entry.connect_activate(move |_| submit());
    dialog.present();
}

fn choose_output(ui: &Rc<Ui>) {
    let Some(input) = ui.state.borrow().path.clone() else { return };
    let suggested = input
        .file_stem()
        .map(|s| format!("{}-booklet.pdf", s.to_string_lossy()))
        .unwrap_or_else(|| "booklet.pdf".to_string());
    let dialog = FileDialog::builder()
        .title("Uložiť prepočítané PDF")
        .initial_name(suggested)
        .filters(&pdf_filter())
        .modal(true)
        .build();
    if let Some(dir) = input.parent().map(gio::File::for_path) {
        dialog.set_initial_folder(Some(&dir));
    }
    let ui = ui.clone();
    dialog.save(Some(&ui.window.clone()), gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(output) = file.path() {
                run_impose(&ui, input.clone(), output);
            }
        }
    });
}

/// Spustí prepočet na vlákne mimo GUI, aby okno nezamrzlo.
fn run_impose(ui: &Rc<Ui>, input: PathBuf, output: PathBuf) {
    let opts = read_options(ui);
    ui.state.borrow_mut().busy = true;
    ui.save_button.set_sensitive(false);
    ui.status.set_text("Prepočítavam…");

    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let out_for_msg = output.clone();
        let result =
            gio::spawn_blocking(move || impose_file(&input, &output, &opts).map(|s| s.plan)).await;
        ui.state.borrow_mut().busy = false;
        match result {
            Ok(Ok(plan)) => {
                ui.status.set_markup(&esc(&format!(
                    "Uložené: {}\n{} listov, {} strán výstupu.",
                    out_for_msg.display(),
                    plan.sheets,
                    plan.output_pages()
                )));
            }
            Ok(Err(e)) => error_dialog(&ui, &format!("{e}")),
            Err(_) => error_dialog(&ui, "prepočet sa nečakane prerušil"),
        }
        let has_plan = ui.state.borrow().plan.as_ref().is_some_and(|p| !p.sides.is_empty());
        ui.save_button.set_sensitive(has_plan);
    });
}

fn error_dialog(ui: &Rc<Ui>, message: &str) {
    ui.status.set_markup(&format!("<span foreground='#c01c28'>{}</span>", esc(message)));
    AlertDialog::builder()
        .message("Nepodarilo sa spracovať PDF")
        .detail(message)
        .modal(true)
        .build()
        .show(Some(&ui.window));
}

/// Presúvanie PDF do okna.
fn enable_drop(ui: &Rc<Ui>) {
    let target = gtk4::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
    let handler = ui.clone();
    target.connect_drop(move |_, value, _, _| {
        if let Ok(file) = value.get::<gio::File>() {
            if let Some(path) = file.path() {
                load_input(&handler, path);
                return true;
            }
        }
        false
    });
    ui.window.add_controller(target);
}

fn esc(text: &str) -> String {
    glib::markup_escape_text(text).to_string()
}
