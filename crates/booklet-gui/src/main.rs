//! GTK4 aplikácia na prepočet PDF do brožúry / zošitov.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod preview;
mod thumbs;

use crate::preview::Preview;
use crate::thumbs::{ThumbSource, Thumbnails};

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use booklet_core::{
    impose_file, info, plan as planner, Binding, Error, Flip, FoldMark, Lang, Marks, Mode, Options,
    Orientation as SheetOrientation, Paper, PdfInfo, Plan, PlanOptions, SheetOrder,
};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    AlertDialog, Application, ApplicationWindow, Box as GBox, Button, CheckButton, DrawingArea,
    DropDown, Entry, FileDialog, FileFilter, Frame, Grid, HeaderBar, Label, Orientation,
    ScrolledWindow, SpinButton, StringList, Widget,
};

const APP_ID: &str = "sk.booklet.BookletPdf";

/// Získavač preloženého textu. Metódy [`Lang`] sa naň dajú priamo použiť.
type Text = fn(Lang) -> &'static str;

const MODES: &[Text] = &[Lang::mode_booklet, Lang::mode_signatures, Lang::mode_two_up];
const ORIENTATIONS: &[Text] = &[Lang::orientation_landscape, Lang::orientation_portrait];
const BINDINGS: &[Text] = &[Lang::binding_left, Lang::binding_right];
const FLIPS: &[Text] = &[Lang::flip_short_edge, Lang::flip_long_edge];
const ORDERS: &[Text] =
    &[Lang::order_interleaved, Lang::order_fronts_then_backs, Lang::order_backs_reversed];
const FOLDS: &[Text] = &[Lang::fold_none, Lang::fold_ticks, Lang::fold_line];

/// Text v rozhraní, ktorý sa má pri zmene jazyka prekresliť.
enum TextSlot {
    Label(Label, Text),
    Markup(Label, Text),
    Check(CheckButton, Text),
    ButtonLabel(Button, Text),
    Placeholder(Entry, Text),
    Tooltip(Widget, Text),
}

impl TextSlot {
    fn apply(&self, lang: Lang) {
        match self {
            TextSlot::Label(w, text) => w.set_label(text(lang)),
            TextSlot::Markup(w, text) => w.set_markup(&format!("<b>{}</b>", esc(text(lang)))),
            TextSlot::Check(w, text) => w.set_label(Some(text(lang))),
            TextSlot::ButtonLabel(w, text) => w.set_label(text(lang)),
            TextSlot::Placeholder(w, text) => w.set_placeholder_text(Some(text(lang))),
            TextSlot::Tooltip(w, text) => w.set_tooltip_text(Some(text(lang))),
        }
    }
}

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
    lang: Cell<Lang>,
    /// Práve prebieha zmena jazyka — prepočet plánu sa má preskočiť.
    retranslating: Cell<bool>,
    texts: RefCell<Vec<TextSlot>>,
    /// Rozbaľovacie zoznamy a získavače ich položiek.
    lists: RefCell<Vec<(DropDown, &'static [Text])>>,
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
    let save_button = Button::with_label("");
    save_button.add_css_class("suggested-action");
    save_button.set_sensitive(false);

    let lang_dd =
        DropDown::from_strings(&Lang::ALL.iter().map(|l| l.endonym()).collect::<Vec<_>>());
    let detected = Lang::detect();
    lang_dd.set_selected(Lang::ALL.iter().position(|l| *l == detected).unwrap_or(0) as u32);

    let header = HeaderBar::new();
    header.pack_start(&open_button);
    header.pack_end(&save_button);
    header.pack_end(&lang_dd);
    window.set_titlebar(Some(&header));

    let area = DrawingArea::new();
    area.set_content_height(400);

    let ui = Rc::new(Ui {
        window: window.clone(),
        file_label: Label::builder().wrap(true).xalign(0.0).max_width_chars(34).build(),
        status: Label::builder().xalign(0.0).wrap(true).max_width_chars(100).build(),
        mode: DropDown::default(),
        sheets: SpinButton::with_range(1.0, 60.0, 1.0),
        sheets_row: GBox::new(Orientation::Horizontal, 8),
        paper: DropDown::default(),
        orientation: DropDown::default(),
        binding: DropDown::default(),
        flip: DropDown::default(),
        order: DropDown::default(),
        margin: SpinButton::with_range(0.0, 50.0, 0.5),
        gutter: SpinButton::with_range(0.0, 60.0, 0.5),
        creep: SpinButton::with_range(0.0, 2.0, 0.05),
        scale: CheckButton::new(),
        show_thumbs: CheckButton::new(),
        fold: DropDown::default(),
        crop: CheckButton::new(),
        range: Entry::new(),
        save_button: save_button.clone(),
        area: area.clone(),
        lang: Cell::new(detected),
        retranslating: Cell::new(false),
        texts: RefCell::new(Vec::new()),
        lists: RefCell::new(Vec::new()),
        state: RefCell::new(State { sheet: (0.0, 0.0), ..State::default() }),
    });

    // Zoznamy sa napĺňajú prekladom, takže najprv treba zaregistrovať
    // získavače a až potom nastaviť predvolené položky.
    {
        let mut lists = ui.lists.borrow_mut();
        lists.push((ui.mode.clone(), MODES));
        lists.push((ui.orientation.clone(), ORIENTATIONS));
        lists.push((ui.binding.clone(), BINDINGS));
        lists.push((ui.flip.clone(), FLIPS));
        lists.push((ui.order.clone(), ORDERS));
        lists.push((ui.fold.clone(), FOLDS));
    }
    {
        let mut texts = ui.texts.borrow_mut();
        texts.push(TextSlot::ButtonLabel(save_button.clone(), Lang::save_button));
        texts.push(TextSlot::Tooltip(open_button.clone().upcast(), Lang::open_tooltip));
        texts.push(TextSlot::Tooltip(lang_dd.clone().upcast(), Lang::language_tooltip));
        texts.push(TextSlot::Check(ui.scale.clone(), Lang::check_scale));
        texts.push(TextSlot::Check(ui.crop.clone(), Lang::check_crop));
        texts.push(TextSlot::Check(ui.show_thumbs.clone(), Lang::check_thumbs));
        texts.push(TextSlot::Tooltip(ui.scale.clone().upcast(), Lang::tip_scale));
        texts.push(TextSlot::Tooltip(ui.show_thumbs.clone().upcast(), Lang::tip_thumbs));
        texts.push(TextSlot::Tooltip(ui.fold.clone().upcast(), Lang::tip_fold));
        texts.push(TextSlot::Tooltip(ui.flip.clone().upcast(), Lang::tip_flip));
        texts.push(TextSlot::Tooltip(ui.creep.clone().upcast(), Lang::tip_creep));
        texts.push(TextSlot::Placeholder(ui.range.clone(), Lang::pages_placeholder));
    }

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
    content.set_vexpand(true);

    // Súhrn patrí do spodnej lišty — v bočnom paneli by sa schoval pod
    // posuvník práve vtedy, keď má čo dôležité povedať.
    ui.status.set_margin_top(8);
    ui.status.set_margin_bottom(8);
    ui.status.set_margin_start(14);
    ui.status.set_margin_end(14);
    let root = GBox::new(Orientation::Vertical, 0);
    root.append(&content);
    root.append(&gtk4::Separator::new(Orientation::Horizontal));
    root.append(&ui.status);
    window.set_child(Some(&root));

    retranslate(&ui);
    ui.mode.set_selected(0);
    ui.orientation.set_selected(0);
    ui.binding.set_selected(0);
    ui.flip.set_selected(0);
    ui.order.set_selected(0);
    ui.fold.set_selected(1);
    ui.paper.set_selected(Paper::ALL.iter().position(|p| *p == Paper::A4).unwrap_or(0) as u32);
    ui.sheets.set_value(4.0);
    ui.creep.set_digits(2);
    ui.margin.set_digits(1);
    ui.gutter.set_digits(1);
    ui.scale.set_active(true);
    ui.show_thumbs.set_active(true);
    ui.sheets.set_hexpand(true);

    // Kreslenie náhľadu.
    {
        let ui = ui.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let (plan, sheet) = {
                let state = ui.state.borrow();
                let plan = state.plan.clone().unwrap_or_else(empty_plan);
                let sheet = if state.sheet.0 > 0.0 { state.sheet } else { (841.89, 595.28) };
                (plan, sheet)
            };
            let thumbs =
                if ui.show_thumbs.is_active() { ui.state.borrow().thumbs.clone() } else { None };
            let opts = read_options(&ui);
            preview::draw(
                cr,
                w as f64,
                h as f64,
                &Preview {
                    plan: &plan,
                    sheet,
                    opts: &opts,
                    lang: ui.lang.get(),
                    thumbs: thumbs.as_deref().map(|t| t as &dyn ThumbSource),
                    device_scale: ui.area.scale_factor().max(1) as f64,
                },
            );
        });
    }
    {
        let ui = ui.clone();
        area.connect_resize(move |_, _, _| resize_preview(&ui));
    }

    // Prepojenie ovládacích prvkov.
    for dd in [&ui.mode, &ui.paper, &ui.orientation, &ui.binding, &ui.flip, &ui.order, &ui.fold] {
        let handler = ui.clone();
        dd.connect_selected_notify(move |_| refresh(&handler));
    }
    for sb in [&ui.sheets, &ui.margin, &ui.gutter, &ui.creep] {
        let handler = ui.clone();
        sb.connect_value_changed(move |_| refresh(&handler));
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
        let handler = ui.clone();
        lang_dd.connect_selected_notify(move |dd| {
            let lang = Lang::ALL.get(dd.selected() as usize).copied().unwrap_or_default();
            handler.lang.set(lang);
            retranslate(&handler);
            refresh(&handler);
        });
    }
    {
        let handler = ui.clone();
        open_button.connect_clicked(move |_| choose_input(&handler));
    }
    {
        let handler = ui.clone();
        save_button.connect_clicked(move |_| choose_output(&handler));
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

/// Prepíše všetky texty do aktuálneho jazyka a zachová vybrané položky.
fn retranslate(ui: &Rc<Ui>) {
    let lang = ui.lang.get();
    ui.retranslating.set(true);

    for binding in ui.texts.borrow().iter() {
        binding.apply(lang);
    }
    for (dd, items) in ui.lists.borrow().iter() {
        let labels: Vec<String> = items.iter().map(|text| text(lang).to_string()).collect();
        set_items(dd, &labels);
    }
    // Formáty ako A4 sú rovnaké v každom jazyku, `podľa zdroja` nie.
    let papers: Vec<String> = Paper::ALL.iter().map(|p| lang.paper_label(*p)).collect();
    set_items(&ui.paper, &papers);

    if ui.state.borrow().info.is_none() {
        ui.file_label.set_text(lang.no_file());
        ui.status.set_text(lang.status_open_pdf());
    } else if let Some((name, pages, size)) = file_summary(ui) {
        ui.file_label.set_text(&format!("{name}\n{}", lang.file_info(pages, size.0, size.1)));
    }

    ui.retranslating.set(false);
}

/// `set_model` zahodí vybranú položku, tak si ju treba odložiť.
///
/// Zoznam bez modelu vracia `selected()` ako „neplatná pozícia“ (`u32::MAX`);
/// bez tohto ošetrenia by sa z nej po obmedzení na dĺžku stala posledná
/// položka a rozhranie by nabehlo s inými hodnotami, než akými sa tvári.
fn set_items(dd: &DropDown, labels: &[String]) {
    let keep = keep_selection(dd.selected(), labels.len());
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    dd.set_model(Some(&StringList::new(&refs)));
    if let Some(index) = keep {
        dd.set_selected(index);
    }
}

/// Ktorú položku vybrať po výmene modelu zoznamu.
fn keep_selection(previous: u32, len: usize) -> Option<u32> {
    if len == 0 {
        return None;
    }
    if previous == gtk4::INVALID_LIST_POSITION {
        return Some(0);
    }
    Some(previous.min(len as u32 - 1))
}

fn file_summary(ui: &Rc<Ui>) -> Option<(String, usize, (f64, f64))> {
    let state = ui.state.borrow();
    let path = state.path.as_ref()?;
    let info = state.info.as_ref()?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (w, h) = info.first_page_pt;
    Some((name, info.pages, (booklet_core::to_mm(w), booklet_core::to_mm(h))))
}

fn sidebar(ui: &Rc<Ui>) -> ScrolledWindow {
    let outer = GBox::new(Orientation::Vertical, 12);
    outer.set_margin_top(14);
    outer.set_margin_bottom(14);
    outer.set_margin_start(14);
    outer.set_margin_end(14);

    outer.append(&section(ui, Lang::section_input));
    let file_box = GBox::new(Orientation::Vertical, 4);
    file_box.append(&ui.file_label);
    outer.append(&framed(&file_box));

    outer.append(&section(ui, Lang::section_folding));
    let grid = new_grid();
    let mut r = 0;
    add_row(ui, &grid, &mut r, Lang::row_mode, &ui.mode);
    ui.sheets_row.append(&ui.sheets);
    add_row(ui, &grid, &mut r, Lang::row_sheets, &ui.sheets_row);
    add_row(ui, &grid, &mut r, Lang::row_binding, &ui.binding);
    add_row(ui, &grid, &mut r, Lang::row_pages, &ui.range);
    outer.append(&framed(&grid));

    outer.append(&section(ui, Lang::section_paper));
    let grid = new_grid();
    let mut r = 0;
    add_row(ui, &grid, &mut r, Lang::row_paper, &ui.paper);
    add_row(ui, &grid, &mut r, Lang::row_orientation, &ui.orientation);
    add_row(ui, &grid, &mut r, Lang::row_margin, &ui.margin);
    add_row(ui, &grid, &mut r, Lang::row_gutter, &ui.gutter);
    add_row(ui, &grid, &mut r, Lang::row_fold, &ui.fold);
    grid.attach(&ui.crop, 0, r, 2, 1);
    r += 1;
    grid.attach(&ui.scale, 0, r, 2, 1);
    r += 1;
    grid.attach(&ui.show_thumbs, 0, r, 2, 1);
    outer.append(&framed(&grid));

    outer.append(&section(ui, Lang::section_printing));
    let grid = new_grid();
    let mut r = 0;
    add_row(ui, &grid, &mut r, Lang::row_flip, &ui.flip);
    add_row(ui, &grid, &mut r, Lang::row_order, &ui.order);
    add_row(ui, &grid, &mut r, Lang::row_creep, &ui.creep);
    outer.append(&framed(&grid));

    ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .hexpand(false)
        .width_request(400)
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

fn add_row(ui: &Rc<Ui>, grid: &Grid, row: &mut i32, text: Text, widget: &impl IsA<gtk4::Widget>) {
    let label = Label::builder().xalign(0.0).build();
    label.add_css_class("dim-label");
    grid.attach(&label, 0, *row, 1, 1);
    ui.texts.borrow_mut().push(TextSlot::Label(label, text));
    let w = widget.as_ref();
    w.set_hexpand(true);
    grid.attach(w, 1, *row, 1, 1);
    *row += 1;
}

fn section(ui: &Rc<Ui>, text: Text) -> Label {
    let label = Label::builder().use_markup(true).xalign(0.0).build();
    ui.texts.borrow_mut().push(TextSlot::Markup(label.clone(), text));
    label
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
    // Počas prekladu sa modely zoznamov vymieňajú a signály by sem chodili
    // s polovične nastaveným rozhraním.
    if ui.retranslating.get() {
        return;
    }
    sync_sensitivity(ui);
    let lang = ui.lang.get();
    let opts = read_options(ui);
    let loaded = ui.state.borrow().info.as_ref().map(|i| (i.pages, i.first_page_pt));
    let Some((pages, first)) = loaded else {
        ui.state.borrow_mut().plan = None;
        ui.status.set_text(lang.status_open_pdf());
        ui.area.queue_draw();
        return;
    };

    let selection = match planner::parse_range(&opts.range, pages) {
        Ok(selection) => selection,
        Err(e) => {
            show_error_text(ui, &lang.range_error(&e));
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
    ui.status.set_text(&status_text(lang, &plan, sheet, &opts));
    resize_preview(ui);
    ui.area.queue_draw();
}

/// Súhrn v spodnej lište. Okrem počtov hlási aj to, či sa rozdelenie na
/// zošity vôbec uplatnilo — inak sa zdá, že voľba nič nerobí.
fn status_text(lang: Lang, plan: &Plan, sheet: (f64, f64), opts: &Options) -> String {
    let mut text = lang.counts(plan.source_pages, plan.sheets, plan.output_pages(), plan.blanks);
    if matches!(opts.plan.mode, Mode::Signatures { .. }) {
        if plan.is_grouped() {
            text.push('\n');
            text.push_str(
                &lang.signatures_summary(plan.signatures.len(), &plan.signature_breakdown()),
            );
            text.push('.');
        } else if !plan.signatures.is_empty() {
            text.push('\n');
            text.push_str(lang.status_one_signature());
        }
    }
    text.push('\n');
    text.push_str(&lang.sheet_size(booklet_core::to_mm(sheet.0), booklet_core::to_mm(sheet.1)));
    text
}

fn empty_plan() -> Plan {
    Plan { sides: vec![], sheets: 0, signatures: vec![], source_pages: 0, blanks: 0 }
}

fn resize_preview(ui: &Rc<Ui>) {
    let state = ui.state.borrow();
    let plan = state.plan.clone().unwrap_or_else(empty_plan);
    let sheet = if state.sheet.0 > 0.0 { state.sheet } else { (841.89, 595.28) };
    drop(state);
    let w = ui.area.width().max(1);
    let h = if plan.sides.is_empty() {
        400.0
    } else {
        preview::layout(&plan, w as f64, sheet, ui.lang.get()).total_h
    };
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
    let dialog = FileDialog::builder()
        .title(ui.lang.get().dialog_open())
        .filters(&pdf_filter())
        .modal(true)
        .build();
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
            let redraw = {
                let area = ui.area.clone();
                move || area.queue_draw()
            };
            let thumbs = Thumbnails::open(path.clone(), password.clone(), redraw);
            {
                let mut state = ui.state.borrow_mut();
                state.path = Some(path);
                state.info = Some(pdf);
                // Starý renderer zaniká spolu s posledným Rc a vlákno skončí.
                state.thumbs = Some(thumbs);
            }
            if let Some((name, pages, size)) = file_summary(ui) {
                let lang = ui.lang.get();
                ui.file_label
                    .set_text(&format!("{name}\n{}", lang.file_info(pages, size.0, size.1)));
            }
            refresh(ui);
        }
        Err(Error::Encrypted) => ask_password(ui, path),
        Err(e) => {
            let message = ui.lang.get().error(&e);
            error_dialog(ui, &message);
        }
    }
}

/// Modálny dotaz na heslo zašifrovaného PDF.
fn ask_password(ui: &Rc<Ui>, path: PathBuf) {
    let lang = ui.lang.get();
    let entry = gtk4::PasswordEntry::builder().show_peek_icon(true).build();
    let content = GBox::new(Orientation::Vertical, 10);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.append(&Label::new(Some(lang.password_prompt())));
    content.append(&entry);
    let buttons = GBox::new(Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    let cancel = Button::with_label(lang.button_cancel());
    let ok = Button::with_label(lang.button_open());
    ok.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&ok);
    content.append(&buttons);

    let dialog = gtk4::Window::builder()
        .title(lang.password_title())
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
        .title(ui.lang.get().dialog_save())
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
    ui.status.set_text(ui.lang.get().status_working());

    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let shown_path = output.display().to_string();
        let result =
            gio::spawn_blocking(move || impose_file(&input, &output, &opts).map(|s| s.plan)).await;
        ui.state.borrow_mut().busy = false;
        let lang = ui.lang.get();
        match result {
            Ok(Ok(plan)) => {
                ui.status.set_text(&lang.saved(&shown_path, plan.sheets, plan.output_pages()));
            }
            Ok(Err(e)) => {
                let message = lang.error(&e);
                error_dialog(&ui, &message);
            }
            Err(_) => error_dialog(&ui, lang.error_interrupted()),
        }
        let has_plan = ui.state.borrow().plan.as_ref().is_some_and(|p| !p.sides.is_empty());
        ui.save_button.set_sensitive(has_plan);
    });
}

fn show_error_text(ui: &Rc<Ui>, message: &str) {
    ui.status.set_markup(&format!("<span foreground='#c01c28'>{}</span>", esc(message)));
}

fn error_dialog(ui: &Rc<Ui>, message: &str) {
    show_error_text(ui, message);
    AlertDialog::builder()
        .message(ui.lang.get().dialog_error())
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

#[cfg(test)]
mod tests {
    use super::keep_selection;

    #[test]
    fn selection_survives_a_model_swap() {
        assert_eq!(keep_selection(2, 3), Some(2));
        assert_eq!(keep_selection(0, 3), Some(0));
    }

    #[test]
    fn invalid_position_falls_back_to_the_first_item() {
        // Zoznam bez modelu — bez tohto by sa vybrala posledná položka.
        assert_eq!(keep_selection(gtk4::INVALID_LIST_POSITION, 3), Some(0));
    }

    #[test]
    fn selection_is_clamped_to_a_shorter_list() {
        assert_eq!(keep_selection(9, 3), Some(2));
        assert_eq!(keep_selection(0, 0), None);
    }
}
