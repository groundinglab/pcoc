//! The window: sidebar, header with the key pill, the page stack and the
//! per-job status bar (DESIGN-v2 sections 3 and 8).

use crate::pages;
use crate::state::{Page, SignInState, State, VerifyState};
use crate::widgets::{self, hbox, line, status_dot, vbox};
use adw::prelude::*;
use gtk::{gio, glib};
use pcoc_hike_core::dm;
use pcoc_hike_core::format::{clock, tilde};
use pcoc_hike_core::keys::{PublicKey, SecretKey};
use pcoc_hike_core::produce::events::ProduceEvent;
use pcoc_hike_core::produce::live::{self, LiveConfig};
use pcoc_hike_core::produce::model::{Phase, ProduceView};
use pcoc_hike_core::signin;
use pcoc_hike_core::verify_job::{self, VerifyConfig};
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

type Ticker = Box<dyn Fn(&State)>;

pub struct App {
    pub state: RefCell<State>,
    pub window: adw::ApplicationWindow,
    css: gtk::CssProvider,
    shell: OnceCell<Shell>,
    tickers: RefCell<HashMap<Page, Vec<Ticker>>>,
    dirty: Cell<bool>,
    /// The Produce raw-log expander. Kept across refreshes so it stays open.
    pub produce_log: RefCell<Option<gtk::Expander>>,
}

struct Shell {
    sidebar: gtk::Box,
    brand_words: gtk::Box,
    nav: gtk::ListBox,
    nav_rows: Vec<(Page, gtk::ListBoxRow, gtk::Label, gtk::Label)>,
    objects_head: gtk::Label,
    objects: gtk::ListBox,
    learn_words: gtk::Box,
    learn_switch: gtk::Switch,
    title: gtk::Label,
    sub: gtk::Label,
    actions: gtk::Box,
    pill: gtk::Button,
    stack: gtk::Stack,
    holders: HashMap<Page, (gtk::ScrolledWindow, gtk::Box)>,
    status: gtk::Box,
}

impl App {
    pub fn new(application: &adw::Application, state: State) -> Rc<App> {
        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title("PCOC Hike")
            .default_width(1280)
            .default_height(720)
            .width_request(360)
            .height_request(480)
            .build();
        let css = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &gtk::prelude::WidgetExt::display(&window),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let app = Rc::new(App {
            state: RefCell::new(state),
            window,
            css,
            shell: OnceCell::new(),
            tickers: RefCell::new(HashMap::new()),
            dirty: Cell::new(false),
            produce_log: RefCell::new(None),
        });
        app.apply_theme();
        app.build_shell();
        app.install_breakpoint();
        app.install_close_guard();
        app.install_ticker();
        let page = app.state.borrow().page;
        app.go(page);
        app
    }

    fn shell(&self) -> &Shell {
        self.shell.get().expect("shell built")
    }

    pub fn apply_theme(&self) {
        let theme = self.state.borrow().config.effective_theme();
        self.css.load_from_string(&crate::style::css(theme));
        adw::StyleManager::default().set_color_scheme(match theme {
            pcoc_hike_core::config::Theme::Dark => adw::ColorScheme::ForceDark,
            pcoc_hike_core::config::Theme::Light => adw::ColorScheme::ForceLight,
        });
    }

    fn build_shell(self: &Rc<Self>) {
        let root = hbox(0);
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_size_request(0, 0);

        // Sidebar. A width request would be a minimum the window cannot
        // shrink past, so the rail width is CSS and the breakpoint.
        let sidebar = vbox(0);
        sidebar.add_css_class("sidebar");
        sidebar.set_hexpand(false);
        sidebar.set_vexpand(true);
        sidebar.set_size_request(0, 0);
        let brand = hbox(10);
        brand.set_margin_start(6);
        brand.set_margin_bottom(14);
        brand.append(&logo());
        let brand_words = vbox(0);
        brand_words.append(&line("PCOC", &["brand-name"]));
        brand_words.append(&line("Satoshi Hike", &["brand-sub"]));
        brand.append(&brand_words);
        sidebar.append(&brand);

        let nav = gtk::ListBox::new();
        nav.add_css_class("nav");
        nav.set_selection_mode(gtk::SelectionMode::Single);
        let mut nav_rows = Vec::new();
        for page in Page::ALL {
            let row = gtk::ListBoxRow::new();
            let b = hbox(10);
            let icon = gtk::Image::from_icon_name(page.icon());
            b.append(&icon);
            let label = line(page.title(), &["nav-label"]);
            label.set_hexpand(true);
            b.append(&label);
            let badge = line("", &["badge"]);
            badge.set_visible(false);
            b.append(&badge);
            row.set_child(Some(&b));
            row.set_tooltip_text(Some(page.title()));
            row.update_property(&[gtk::accessible::Property::Label(page.title())]);
            nav.append(&row);
            nav_rows.push((page, row, label, badge));
        }
        {
            let weak = Rc::downgrade(self);
            nav.connect_row_activated(move |_, row| {
                if let Some(app) = weak.upgrade() {
                    let page = Page::ALL[row.index() as usize];
                    app.go(page);
                }
            });
        }
        sidebar.append(&nav);

        let objects_head = line("HIKE OBJECTS", &["side-head"]);
        sidebar.append(&objects_head);
        let objects = gtk::ListBox::new();
        objects.add_css_class("objects");
        objects.set_selection_mode(gtk::SelectionMode::Single);
        {
            let weak = Rc::downgrade(self);
            objects.connect_row_activated(move |_, row| {
                if let Some(app) = weak.upgrade() {
                    let name = row.widget_name();
                    if !name.is_empty() {
                        app.open_hike(PathBuf::from(name.as_str()));
                    }
                }
            });
        }
        let objects_scroll = gtk::ScrolledWindow::builder()
            .child(&objects)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        sidebar.append(&objects_scroll);

        let learn_row = hbox(8);
        learn_row.set_margin_start(6);
        learn_row.set_margin_end(4);
        let learn_words = vbox(0);
        learn_words.set_hexpand(true);
        learn_words.append(&line("Learn mode", &["strong", "small"]));
        learn_words.append(&line("SPEC terms on", &["muted", "small"]));
        learn_row.append(&learn_words);
        let learn_switch = gtk::Switch::new();
        learn_switch.set_valign(gtk::Align::Center);
        learn_switch.set_tooltip_text(Some(
            "Learn mode: SPEC terms and one-line explanations on these same screens",
        ));
        learn_switch.update_property(&[gtk::accessible::Property::Label("Learn mode")]);
        learn_switch.set_active(self.state.borrow().learn());
        {
            let weak = Rc::downgrade(self);
            learn_switch.connect_active_notify(move |s| {
                let on = s.is_active();
                let weak = weak.clone();
                // Refresh after the signal returns. Rebuilding the page inside
                // the notify destroys widgets the signal still holds.
                glib::idle_add_local_once(move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_learn(on);
                    }
                });
            });
        }
        learn_row.append(&learn_switch);
        sidebar.append(&learn_row);
        root.append(&sidebar);

        // Main column.
        let main = vbox(0);
        main.set_hexpand(true);
        main.set_vexpand(true);
        main.set_size_request(0, 0);
        let header = adw::HeaderBar::new();
        header.set_show_title(false);
        let title_box = hbox(6);
        title_box.set_margin_start(8);
        title_box.set_hexpand(true);
        let title = line("", &["page-title"]);
        let sub = line("", &["page-sub"]);
        sub.set_hexpand(true);
        sub.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        title_box.append(&title);
        title_box.append(&sub);
        header.pack_start(&title_box);
        let pill = gtk::Button::new();
        pill.add_css_class("flat");
        {
            let weak = Rc::downgrade(self);
            pill.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.go(Page::Produce);
                }
            });
        }
        header.pack_end(&pill);
        let actions = hbox(8);
        header.pack_end(&actions);
        main.append(&header);

        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::None);
        let mut holders = HashMap::new();
        for page in Page::ALL {
            let holder = vbox(0);
            holder.set_hexpand(true);
            let scroll = gtk::ScrolledWindow::builder()
                .child(&holder)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .propagate_natural_width(false)
                .hexpand(true)
                .vexpand(true)
                .build();
            stack.add_named(&scroll, Some(page.name()));
            holders.insert(page, (scroll, holder));
        }
        main.append(&stack);

        let status = hbox(10);
        status.add_css_class("statusbar");
        status.set_hexpand(true);
        main.append(&status);
        root.append(&main);

        self.window.set_content(Some(&root));
        let _ = self.shell.set(Shell {
            sidebar,
            brand_words,
            nav,
            nav_rows,
            objects_head,
            objects,
            learn_words,
            learn_switch,
            title,
            sub,
            actions,
            pill,
            stack,
            holders,
            status,
        });
    }

    fn install_breakpoint(self: &Rc<Self>) {
        let condition =
            adw::BreakpointCondition::parse("max-width: 1279px").expect("breakpoint condition");
        let bp = adw::Breakpoint::new(condition);
        let weak = Rc::downgrade(self);
        bp.connect_apply(move |_| {
            if let Some(app) = weak.upgrade() {
                app.set_narrow(true);
            }
        });
        let weak = Rc::downgrade(self);
        bp.connect_unapply(move |_| {
            if let Some(app) = weak.upgrade() {
                app.set_narrow(false);
            }
        });
        self.window.add_breakpoint(bp);
    }

    fn set_narrow(self: &Rc<Self>, narrow: bool) {
        self.state.borrow_mut().narrow = narrow;
        let shell = self.shell();
        if narrow {
            shell.sidebar.add_css_class("rail");
        } else {
            shell.sidebar.remove_css_class("rail");
        }
        shell.brand_words.set_visible(!narrow);
        shell.objects_head.set_visible(!narrow);
        shell.objects.set_visible(!narrow);
        shell.learn_words.set_visible(!narrow);
        for (_, _, label, _) in &shell.nav_rows {
            label.set_visible(!narrow);
        }
        self.refresh_all();
    }

    fn install_close_guard(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |window| {
            let Some(app) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !app.state.borrow().produce.running() {
                app.state.borrow_mut().secret = None;
                return glib::Propagation::Proceed;
            }
            let dialog = stop_dialog("Quit during a hike?", "Quit");
            let weak = Rc::downgrade(&app);
            let window = window.clone();
            dialog.choose(&app.window, None::<&gio::Cancellable>, move |response| {
                if response == "stop" {
                    if let Some(app) = weak.upgrade() {
                        if let Some(run) = app.state.borrow().produce_run.as_ref() {
                            run.stop();
                        }
                        app.state.borrow_mut().secret = None;
                        app.state.borrow_mut().produce.phase = Phase::Stopped;
                    }
                    window.destroy();
                }
            });
            glib::Propagation::Stop
        });
    }

    fn install_ticker(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::timeout_add_seconds_local(1, move || {
            let Some(app) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let page = app.state.borrow().page;
            if let Some(tickers) = app.tickers.borrow().get(&page) {
                let state = app.state.borrow();
                for t in tickers {
                    t(&state);
                }
            }
            app.refresh_status();
            glib::ControlFlow::Continue
        });
    }

    /// Register a label update that runs once a second while `page` is shown.
    pub fn tick(&self, page: Page, f: impl Fn(&State) + 'static) {
        self.tickers
            .borrow_mut()
            .entry(page)
            .or_default()
            .push(Box::new(f));
    }

    pub fn go(self: &Rc<Self>, page: Page) {
        if page == Page::Verify {
            let mut state = self.state.borrow_mut();
            if matches!(state.verify, VerifyState::Empty) {
                if let Some(path) = state
                    .produce
                    .object
                    .as_ref()
                    .map(|object| object.path.clone())
                {
                    state.verify = VerifyState::Selected(path);
                }
            }
        }
        self.state.borrow_mut().page = page;
        let shell = self.shell();
        shell.stack.set_visible_child_name(page.name());
        if let Some((_, row, _, _)) = shell.nav_rows.iter().find(|(p, ..)| *p == page) {
            shell.nav.select_row(Some(row));
        }
        self.refresh_page(page);
        self.refresh_chrome();
    }

    pub fn refresh_all(self: &Rc<Self>) {
        let page = self.state.borrow().page;
        self.refresh_page(page);
        self.refresh_chrome();
    }

    /// Coalesce bursts of worker events into one rebuild.
    pub fn refresh_soon(self: &Rc<Self>) {
        if self.dirty.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
            if let Some(app) = weak.upgrade() {
                app.dirty.set(false);
                app.refresh_all();
            }
        });
    }

    pub fn refresh_page(self: &Rc<Self>, page: Page) {
        let Some((scroll, holder)) = self.shell().holders.get(&page) else {
            return;
        };
        // Produce during a run keeps its widgets. Rebuilding them drops the
        // raw-log expander and any tooltip the pointer is holding.
        if page == Page::Produce && self.produce_kept(holder) {
            return;
        }
        let keep = scroll.vadjustment().value();
        self.tickers.borrow_mut().remove(&page);
        if page == Page::Produce {
            *self.produce_log.borrow_mut() = None;
        }
        widgets::clear(holder);
        let content = match page {
            Page::Produce => pages::produce::build(self),
            Page::Verify => pages::verify::build(self),
            Page::SignIn => pages::signin::build(self),
            Page::Settings => pages::settings::build(self),
        };
        holder.append(&content);
        let adj = scroll.vadjustment();
        glib::idle_add_local_once(move || adj.set_value(keep));
    }

    /// True when the Produce page is already showing this run's live view.
    fn produce_kept(&self, holder: &gtk::Box) -> bool {
        let state = self.state.borrow();
        if state.result_path.is_some() || !state.produce.running() {
            return false;
        }
        holder
            .first_child()
            .is_some_and(|child| child.widget_name().as_str() == "produce-live")
    }

    pub fn refresh_chrome(self: &Rc<Self>) {
        self.refresh_sidebar();
        self.refresh_header();
        self.refresh_status();
    }

    fn refresh_sidebar(self: &Rc<Self>) {
        let shell = self.shell();
        let state = self.state.borrow();
        for (page, _, _, badge) in &shell.nav_rows {
            let text = if *page == Page::Produce && !state.narrow {
                state.produce.sidebar_badge()
            } else {
                None
            };
            badge.set_visible(text.is_some());
            badge.set_text(text.as_deref().unwrap_or(""));
        }
        if shell.learn_switch.is_active() != state.learn() {
            shell.learn_switch.set_active(state.learn());
        }
        while let Some(row) = shell.objects.row_at_index(0) {
            shell.objects.remove(&row);
        }
        let rows = hike_rows(&state);
        let selected = state.result_path.clone().or_else(|| {
            state
                .produce
                .running()
                .then(|| state.produce.output.clone())
                .flatten()
        });
        for row_info in rows {
            let row = gtk::ListBoxRow::new();
            let b = vbox(1);
            let n = line(&row_info.name, &["obj-name"]);
            n.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            b.append(&n);
            let status = row_info.status.label();
            b.append(&line(status, &["obj-line"]));
            row.set_child(Some(&b));
            row.update_property(&[gtk::accessible::Property::Label(&format!(
                "{}, {status}",
                row_info.name
            ))]);
            row.set_widget_name(&row_info.path.to_string_lossy());
            row.set_tooltip_text(Some(&format!(
                "{}\n{}",
                tilde(&row_info.path),
                row_info.status.label()
            )));
            shell.objects.append(&row);
            if selected.as_ref() == Some(&row_info.path) {
                shell.objects.select_row(Some(&row));
            }
        }
    }

    fn refresh_header(self: &Rc<Self>) {
        let shell = self.shell();
        let state = self.state.borrow();
        shell.title.set_text(state.page.title());
        let sub = match state.page {
            Page::Produce => produce_subtitle(&state),
            Page::Verify => match &state.verify {
                VerifyState::Done(_) => "· section 14 record".to_owned(),
                VerifyState::Running(_) => "· measuring".to_owned(),
                VerifyState::IdleError { .. } => "· file not measured".to_owned(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        shell.sub.set_text(&sub);
        widgets::clear(&shell.actions);
        match state.page {
            Page::Produce if state.produce.running() => {
                let label = if state.produce_run.as_ref().is_some_and(|r| r.stopping()) {
                    "Stopping…"
                } else {
                    "Stop hike…"
                };
                let b = widgets::button(label, "danger");
                b.set_sensitive(!state.produce_run.as_ref().is_some_and(|r| r.stopping()));
                let weak = Rc::downgrade(self);
                b.connect_clicked(move |_| {
                    if let Some(app) = weak.upgrade() {
                        app.confirm_stop();
                    }
                });
                shell.actions.append(&b);
            }
            Page::Verify => {
                if let VerifyState::Done(run) = &state.verify {
                    if let verify_job::Outcome::Record { json, .. } = &run.outcome {
                        let b = widgets::button("Save record", "outline");
                        let weak = Rc::downgrade(self);
                        let json = json.clone();
                        let suggested = run.path.with_extension("record.json");
                        b.connect_clicked(move |_| {
                            if let Some(app) = weak.upgrade() {
                                app.save_record(json.clone(), suggested.clone());
                            }
                        });
                        shell.actions.append(&b);
                    }
                }
            }
            _ => {}
        }
        shell.pill.set_child(Some(&pill(&state)));
        shell.pill.set_tooltip_text(Some(if state.secret.is_some() {
            "The nsec is in memory for this run of the app. It is not written to disk."
        } else {
            "No nsec yet. Paste or generate one on Produce or Sign-in."
        }));
    }

    pub fn refresh_status(self: &Rc<Self>) {
        let shell = self.shell();
        let state = self.state.borrow();
        let now = state.now_ms();
        let quiet = "No job running. Verify uses the portable index.";
        let (dot, left, right): (Option<&str>, String, String) = match state.page {
            Page::Produce if state.produce.phase != Phase::Idle => (
                Some("amber"),
                state.produce.status_bar_line(now, state.narrow),
                String::new(),
            ),
            Page::Verify => match &state.verify {
                VerifyState::Done(run) => (
                    Some("blue"),
                    format!(
                        "{} · Measured in {} s · {} threads · {} UTC",
                        run.reads.status(pcoc_hike_core::endpoints::INDEX),
                        run.elapsed_s,
                        run.threads,
                        clock(run.finished_unix_s)
                    ),
                    String::new(),
                ),
                VerifyState::Running(_) => (
                    Some("blue"),
                    format!(
                        "Portable index · {} · reading · Measuring",
                        pcoc_hike_core::endpoints::INDEX.trim_start_matches("https://")
                    ),
                    String::new(),
                ),
                VerifyState::IdleError { message, .. } => (None, message.clone(), String::new()),
                _ => (None, quiet.into(), String::new()),
            },
            Page::SignIn if matches!(state.sign_in, SignInState::Sending) => (
                Some("blue"),
                "Submitting once to the admission gate.".into(),
                String::new(),
            ),
            Page::SignIn => {
                let words = match &state.sign_in {
                    SignInState::Done(words) => words.clone(),
                    SignInState::Bad(why) => why.sentence().to_owned(),
                    _ => quiet.into(),
                };
                let dot = if words.starts_with("Pass") {
                    Some("teal")
                } else if words.starts_with("Fail") {
                    None
                } else if words.starts_with("Waiting") {
                    Some("amber")
                } else {
                    None
                };
                (dot, words, String::new())
            }
            _ if state.produce.running() => (
                Some("amber"),
                state.produce.status_bar_line(now, state.narrow),
                String::new(),
            ),
            _ => (None, quiet.into(), String::new()),
        };
        drop(state);
        // One label, updated in place, so a hover on the status line survives a tick.
        let label = match shell.status.first_child().and_then(|c| c.next_sibling()) {
            Some(widget) => widget
                .downcast::<gtk::Label>()
                .expect("status line is a label"),
            None => {
                widgets::clear(&shell.status);
                shell.status.append(&status_dot(None));
                let label = line("", &[]);
                label.set_hexpand(true);
                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                shell.status.append(&label);
                label
            }
        };
        let dot_widget = shell
            .status
            .first_child()
            .expect("status dot")
            .downcast::<gtk::Box>()
            .expect("status dot");
        for class in ["amber", "blue", "teal"] {
            dot_widget.remove_css_class(class);
        }
        if let Some(class) = dot {
            dot_widget.add_css_class(class);
        }
        label.set_text(&left);
        if !right.is_empty() && !left.is_empty() {
            label.set_text(&format!("{left} · {right}"));
        } else if !right.is_empty() {
            label.set_text(&right);
        }
    }

    pub fn set_learn(self: &Rc<Self>, on: bool) {
        {
            let mut s = self.state.borrow_mut();
            if s.config.learn_mode == on {
                return;
            }
            s.config.learn_mode = on;
            s.save_config();
        }
        self.refresh_all();
    }

    pub fn set_theme(self: &Rc<Self>, theme: pcoc_hike_core::config::Theme) {
        {
            let mut s = self.state.borrow_mut();
            s.config.theme = Some(theme);
            s.save_config();
        }
        self.apply_theme();
        self.refresh_all();
    }

    /// Update the session nsec from a field. Does not rebuild the page.
    pub fn show_nsec(
        self: &Rc<Self>,
        text: &str,
        npub: &gtk::Label,
        hex: &gtk::Label,
        err: &gtk::Label,
    ) {
        let trimmed = text.trim();
        let mut state = self.state.borrow_mut();
        if trimmed.is_empty() {
            state.secret = None;
            state.key_error = None;
            drop(state);
            npub.set_text("—");
            hex.set_text("—");
            err.set_visible(false);
            return;
        }
        match SecretKey::from_nsec(trimmed) {
            Ok(secret) => {
                let public = secret.public_key();
                npub.set_text(&public.npub());
                hex.set_text(&public.hex());
                err.set_visible(false);
                state.secret = Some(secret);
                state.key_error = None;
            }
            Err(_) => {
                state.secret = None;
                npub.set_text("—");
                hex.set_text("—");
                // A short nsec prefix is still being typed. Anything else is a bad nsec.
                let still_typing =
                    trimmed.starts_with(pcoc_hike_core::keys::NSEC_PREFIX) && trimmed.len() < 58;
                if still_typing {
                    state.key_error = None;
                    err.set_visible(false);
                } else {
                    state.key_error = Some("Fail. This is not an nsec.".into());
                    err.set_text("Fail. This is not an nsec.");
                    err.set_visible(true);
                }
            }
        }
    }

    pub fn select_object(self: &Rc<Self>, path: PathBuf) {
        let running = matches!(self.state.borrow().verify, VerifyState::Running(_));
        if !running {
            self.state.borrow_mut().verify = VerifyState::Selected(path);
        }
        self.go(Page::Verify);
    }

    /// Verify the PCOC from the finished view. The file is selected, not measured yet.
    pub fn verify_pcoc(self: &Rc<Self>, path: PathBuf) {
        self.select_object(path);
    }

    /// Sign-in with the session nsec and this PCOC. The nsec stays in memory.
    pub fn sign_in_with(self: &Rc<Self>, path: PathBuf) {
        {
            let mut state = self.state.borrow_mut();
            state.sign_in_path = Some(path);
            if matches!(state.sign_in, SignInState::Done(_)) {
                state.sign_in = SignInState::Empty;
                state.sign_in_text.clear();
            }
        }
        self.go(Page::SignIn);
    }

    /// Send to my phone. An explicit press only: Produce finishing never
    /// sends. The event is built and signed here, on the GTK thread, so the
    /// nsec never leaves `State`; only the signed event goes to the worker.
    pub fn send_to_phone(self: &Rc<Self>, path: PathBuf) {
        if self.state.borrow().phone_busy || self.state.borrow().sample {
            return;
        }
        let note = |this: &Rc<Self>, words: String| {
            this.state.borrow_mut().phone_note = Some(words);
            this.refresh_all();
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => return note(self, dm::DmError::Read(err.to_string()).to_string()),
        };
        let prepared = {
            let s = self.state.borrow();
            match s.secret.as_ref() {
                None => Err(dm::DmError::Locked),
                Some(secret) => dm::prepare(&bytes, secret, s.now_s()),
            }
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(err) => return note(self, err.to_string()),
        };
        {
            let mut s = self.state.borrow_mut();
            s.phone_busy = true;
            s.phone_note = Some(format!(
                "Sending this PCOC to {} over Nostr relays.",
                prepared.recipient.npub()
            ));
        }
        self.refresh_all();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("pcoc-send-phone".into())
            .spawn(move || {
                let words = match dm::publish(&prepared, &dm::RELAYS) {
                    Ok(sent) => sent.sentence(),
                    Err(err) => err.to_string(),
                };
                let _ = tx.send_blocking(words);
            })
            .expect("spawn send thread");
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            if let Ok(words) = rx.recv().await {
                if let Some(app) = weak.upgrade() {
                    {
                        let mut s = app.state.borrow_mut();
                        s.phone_busy = false;
                        s.phone_note = Some(words);
                    }
                    app.refresh_all();
                }
            }
        });
    }

    fn open_hike(self: &Rc<Self>, path: PathBuf) {
        if !self.state.borrow().phone_busy {
            self.state.borrow_mut().phone_note = None;
        }
        let status = {
            let state = self.state.borrow();
            hike_rows(&state)
                .into_iter()
                .find(|row| row.path == path)
                .map(|row| row.status)
        };
        use pcoc_hike_core::library::HikeStatus;
        match status {
            Some(HikeStatus::InProgress) => {
                self.state.borrow_mut().result_path = None;
                self.go(Page::Produce);
            }
            Some(HikeStatus::Finished) => {
                self.state.borrow_mut().result_path = Some(path);
                self.go(Page::Produce);
            }
            _ => {
                if !self.state.borrow().produce.running() {
                    self.state.borrow_mut().result_path = None;
                    self.go(Page::Produce);
                }
            }
        }
    }

    // Produce.

    /// Start a hike in a fresh run directory. The nsec must already be in memory.
    pub fn start_hike(self: &Rc<Self>) {
        if self.state.borrow().produce.running() {
            return;
        }
        let prepared = {
            let s = self.state.borrow();
            s.secret.as_ref().map(|key| {
                let public = key.public_key();
                (
                    public.npub(),
                    public.0,
                    s.config.data_folder.clone(),
                    s.config.gossip_peer.trim().to_owned(),
                    s.now_s(),
                )
            })
        };
        let Some((npub, public_key, folder, peer, now)) = prepared else {
            let mut state = self.state.borrow_mut();
            if state.key_error.is_none() {
                state.key_error = Some("Fail. Paste an nsec or press Generate.".into());
            }
            drop(state);
            self.refresh_all();
            return;
        };
        let dir = pcoc_hike_core::format::unique_run_dir(&folder, now, &npub);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.state.borrow_mut().key_error =
                Some(format!("Fail. Could not create the run folder: {err}."));
            self.refresh_all();
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.key_error = None;
            state.result_path = None;
        }
        let output = dir.join(pcoc_hike_core::format::PCOC_FILE_NAME);
        self.start_produce(output, public_key, (!peer.is_empty()).then_some(peer));
    }

    pub fn start_produce(
        self: &Rc<Self>,
        output: PathBuf,
        public_key: [u8; 32],
        peer: Option<String>,
    ) {
        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let (tx, rx) = async_channel::unbounded::<ProduceEvent>();
        let emit: live::Emit = Arc::new(move |e| {
            let _ = tx.send_blocking(e);
        });
        let run = live::start(LiveConfig::new(public_key, output.clone(), peer), emit);
        {
            let mut s = self.state.borrow_mut();
            let now = s.now_ms();
            let mut produce = ProduceView {
                phase: Phase::Starting,
                output: Some(output.clone()),
                started_ms: Some(now),
                ..ProduceView::default()
            };
            produce.stages[0].open = true;
            produce.stages[0].legs = Some(0);
            s.produce = produce;
            s.produce_output = Some(output);
            s.produce_run = Some(run);
        }
        self.listen_produce(rx);
        self.refresh_all();
    }

    pub fn listen_produce(self: &Rc<Self>, rx: async_channel::Receiver<ProduceEvent>) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                let Some(app) = weak.upgrade() else {
                    return;
                };
                match event {
                    ProduceEvent::Stopped => {
                        {
                            let mut s = app.state.borrow_mut();
                            s.produce.phase = Phase::Idle;
                            s.produce_run = None;
                            s.result_path = None;
                            s.rescan();
                        }
                        app.reset_produce();
                    }
                    ProduceEvent::Failed { message } => {
                        let mut s = app.state.borrow_mut();
                        s.produce = ProduceView {
                            phase: Phase::Failed(message),
                            ..Default::default()
                        };
                        s.produce_output = None;
                        s.produce_run = None;
                        s.result_path = None;
                        s.rescan();
                        drop(s);
                        app.refresh_all();
                    }
                    other => {
                        let written = matches!(other, ProduceEvent::ObjectWritten { .. });
                        app.state.borrow_mut().produce.apply(other);
                        if written {
                            let mut s = app.state.borrow_mut();
                            s.produce_run = None;
                            s.result_path = None;
                            s.rescan();
                        }
                        app.refresh_soon();
                    }
                }
            }
        });
    }

    fn confirm_stop(self: &Rc<Self>) {
        let dialog = stop_dialog("Stop this hike?", "Stop hike");
        let weak = Rc::downgrade(self);
        dialog.choose(&self.window, None::<&gio::Cancellable>, move |response| {
            if response != "stop" {
                return;
            }
            if let Some(app) = weak.upgrade() {
                if let Some(run) = app.state.borrow().produce_run.as_ref() {
                    run.stop();
                }
                app.refresh_all();
            }
        });
    }

    pub fn reset_produce(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            if s.produce.running() {
                return;
            }
            s.produce = Default::default();
            s.produce_output = None;
        }
        *self.produce_log.borrow_mut() = None;
        self.refresh_all();
    }

    // Verify.

    pub fn start_verify(self: &Rc<Self>, path: PathBuf) {
        let threads = self.state.borrow().config.threads();
        // A file this verifier cannot decode never starts a measurement.
        // Verify returns to idle with one error line, and the next file can
        // be opened without a restart.
        match std::fs::read(&path) {
            Err(err) => {
                self.state.borrow_mut().verify = VerifyState::IdleError {
                    message: format!("Fail. The file could not be opened: {err}."),
                    path,
                };
                self.refresh_all();
                return;
            }
            Ok(bytes) if !verify_job::object_bytes_are_measurable(&bytes) => {
                self.state.borrow_mut().verify = VerifyState::IdleError {
                    message: verify_job::unreadable_object_line(&path),
                    path,
                };
                self.refresh_all();
                return;
            }
            Ok(_) => {}
        }
        self.state.borrow_mut().verify = VerifyState::Running(path.clone());
        let (tx, rx) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("pcoc-verify".into())
            .spawn(move || {
                let run = verify_job::run(&path, &VerifyConfig::production(threads));
                let _ = tx.send_blocking(run);
            })
            .expect("spawn verify thread");
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            if let Ok(run) = rx.recv().await {
                if let Some(app) = weak.upgrade() {
                    app.state.borrow_mut().verify = match &run.outcome {
                        verify_job::Outcome::CouldNotOpen(err) => VerifyState::IdleError {
                            message: format!("Fail. The file could not be opened: {err}."),
                            path: run.path.clone(),
                        },
                        verify_job::Outcome::Unreadable => VerifyState::IdleError {
                            message: verify_job::unreadable_object_line(&run.path),
                            path: run.path.clone(),
                        },
                        verify_job::Outcome::Record { .. } => VerifyState::Done(Box::new(run)),
                    };
                    app.refresh_all();
                }
            }
        });
        self.refresh_all();
    }

    pub fn open_object_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Open a PCOC")
            .modal(true)
            .build();
        let folder = self.state.borrow().config.data_folder.clone();
        if folder.is_dir() {
            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
        }
        let weak = Rc::downgrade(self);
        dialog.open(Some(&self.window), None::<&gio::Cancellable>, move |res| {
            if let (Some(app), Ok(file)) = (weak.upgrade(), res) {
                if let Some(path) = file.path() {
                    app.select_object(path);
                }
            }
        });
    }

    fn save_record(self: &Rc<Self>, json: String, suggested: PathBuf) {
        let dialog = gtk::FileDialog::builder()
            .title("Save the section 14 record")
            .modal(true)
            .build();
        if let Some(parent) = suggested.parent() {
            dialog.set_initial_folder(Some(&gio::File::for_path(parent)));
        }
        if let Some(name) = suggested.file_name() {
            dialog.set_initial_name(Some(&name.to_string_lossy()));
        }
        dialog.save(Some(&self.window), None::<&gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    if let Err(err) = verify_job::save_record(&path, &json) {
                        eprintln!("pcoc-hike: save record: {err}");
                    }
                }
            }
        });
    }

    // Sign-in.

    pub fn set_sign_in_text(self: &Rc<Self>, text: String) {
        let changed = {
            let mut s = self.state.borrow_mut();
            if s.sign_in_text == text && !matches!(s.sign_in, SignInState::Done(_)) {
                false
            } else {
                s.sign_in = if text.trim().is_empty() {
                    SignInState::Empty
                } else {
                    match signin::parse_link(&text) {
                        Ok(challenge) => SignInState::Parsed(challenge),
                        Err(err) => SignInState::Bad(err),
                    }
                };
                s.sign_in_text = text;
                true
            }
        };
        if changed {
            self.refresh_all();
        }
    }

    /// The one press. Signs the pasted link with the nsec and the chosen PCOC.
    pub fn sign_and_submit(self: &Rc<Self>) {
        let job = self.prepare_submission();
        let Some((submission, nonce, sample)) = job else {
            return;
        };
        self.state.borrow_mut().sign_in = SignInState::Sending;
        self.refresh_all();
        if sample {
            return;
        }
        self.spawn_gate(submission, nonce, true);
    }

    /// Test admission of the produced PCOC, or the newest hike, with the same nsec.
    pub fn test_admission(self: &Rc<Self>) {
        if self.state.borrow().test_busy || self.state.borrow().sample {
            return;
        }
        let Some(path) = self.state.borrow().admission_target() else {
            self.state.borrow_mut().test_note =
                Some("Fail. Produce a PCOC first. Nothing was sent.".into());
            self.refresh_all();
            return;
        };
        self.state.borrow_mut().sign_in_path = Some(path);
        let now = self.state.borrow().now_s();
        let challenge = signin::mint_challenge(now);
        let job = self.prepare_submission_with(challenge, true);
        let Some((submission, nonce, _)) = job else {
            return;
        };
        self.state.borrow_mut().test_busy = true;
        self.state.borrow_mut().test_note = Some("Submitting this PCOC to the gate.".into());
        self.refresh_all();
        self.spawn_gate(submission, nonce, false);
    }

    fn prepare_submission(self: &Rc<Self>) -> Option<(signin::Submission, [u8; 32], bool)> {
        let SignInState::Parsed(challenge) = self.state.borrow().sign_in.clone() else {
            return None;
        };
        self.prepare_submission_with(challenge, false)
    }

    fn prepare_submission_with(
        self: &Rc<Self>,
        challenge: signin::Challenge,
        for_test: bool,
    ) -> Option<(signin::Submission, [u8; 32], bool)> {
        let note = |this: &Rc<Self>, words: String| {
            if for_test {
                this.state.borrow_mut().test_note = Some(words);
            } else {
                this.state.borrow_mut().sign_in = SignInState::Done(words);
            }
            this.refresh_all();
        };
        let (path, sample) = {
            let s = self.state.borrow();
            (s.sign_in_path.clone(), s.sample)
        };
        let Some(path) = path else {
            note(self, "Fail. Choose a PCOC. Nothing was signed.".into());
            return None;
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                note(self, format!("Fail. The file could not be opened: {err}."));
                return None;
            }
        };
        if !pcoc_hike_core::verify_job::object_bytes_are_measurable(&bytes) {
            note(
                self,
                "Fail. This file is not a PCOC this app can read. Nothing was signed.".into(),
            );
            return None;
        }
        let Some(object_key) = pcoc_hike_core::library::Entry::from_bytes(path, &bytes, 0)
            .public_key
            .map(PublicKey)
        else {
            note(
                self,
                "Fail. This file is not a PCOC this app can read. Nothing was signed.".into(),
            );
            return None;
        };
        let secret_ok = self.state.borrow().secret.is_some();
        if !secret_ok {
            note(self, signin::CannotSign::Locked.sentence().to_owned());
            return None;
        }
        let mut aux = [0u8; 32];
        if getrandom::getrandom(&mut aux).is_err() {
            note(
                self,
                "Fail. The app could not read randomness. Nothing was signed.".into(),
            );
            return None;
        }
        let submission = {
            let s = self.state.borrow();
            let secret = s.secret.as_ref()?;
            signin::sign(&challenge, &bytes, object_key, secret, &aux)
        };
        match submission {
            Ok(submission) => Some((submission, challenge.nonce, sample)),
            Err(err) => {
                note(self, err.sentence().to_owned());
                None
            }
        }
    }

    fn spawn_gate(self: &Rc<Self>, submission: signin::Submission, nonce: [u8; 32], sign_in: bool) {
        let (tx, rx) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("pcoc-sign-in".into())
            .spawn(move || {
                let words = signin::submit_and_read(&submission, &nonce);
                let _ = tx.send_blocking(words);
            })
            .expect("spawn sign-in thread");
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            if let Ok(words) = rx.recv().await {
                if let Some(app) = weak.upgrade() {
                    {
                        let mut state = app.state.borrow_mut();
                        if sign_in {
                            state.sign_in = SignInState::Done(words);
                        } else {
                            state.test_busy = false;
                            state.test_note = Some(words);
                        }
                    }
                    app.refresh_all();
                }
            }
        });
    }
}

fn produce_subtitle(state: &State) -> String {
    let p = &state.produce;
    match &p.phase {
        Phase::Idle => String::new(),
        _ if state.narrow && p.running() => {
            format!("· {} of 18 stages recorded", p.stages_recorded())
        }
        Phase::Written => p
            .object
            .as_ref()
            .map(|o| format!("· object written {} UTC", clock(o.at_ms / 1000)))
            .unwrap_or_default(),
        Phase::Stopped | Phase::Failed(_) => "· stopped · no object written".to_owned(),
        _ => {
            let out = p.output.as_ref().map(|o| tilde(o)).unwrap_or_default();
            let started = p
                .started_ms
                .map(|ms| format!(" · started {} UTC", clock(ms / 1000)))
                .unwrap_or_default();
            format!("· {out} · not written yet{started}")
        }
    }
}

fn hike_rows(state: &State) -> Vec<pcoc_hike_core::library::HikeRow> {
    use pcoc_hike_core::library::{HikeRow, HikeStatus};
    let running = state
        .produce
        .running()
        .then(|| state.produce.output.clone())
        .flatten();
    if state.sample {
        let mut rows = Vec::new();
        if let Some(out) = &running {
            if !state.library.iter().any(|entry| &entry.path == out) {
                rows.push(HikeRow {
                    name: hike_label(out),
                    path: out.clone(),
                    status: HikeStatus::InProgress,
                });
            }
        }
        for entry in &state.library {
            let status = if running.as_ref() == Some(&entry.path) {
                HikeStatus::InProgress
            } else {
                HikeStatus::Finished
            };
            rows.push(HikeRow {
                name: entry.file_name(),
                path: entry.path.clone(),
                status,
            });
        }
        return rows;
    }
    pcoc_hike_core::library::scan_hikes(&state.config.data_folder, running.as_deref())
}

fn hike_label(path: &std::path::Path) -> String {
    if path.file_name().is_some_and(|name| name == "pcoc.cbor") {
        if let Some(parent) = path.parent().and_then(|parent| parent.file_name()) {
            return parent.to_string_lossy().into_owned();
        }
    }
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn pill(state: &State) -> gtk::Box {
    let b = hbox(8);
    b.add_css_class("pill");
    let pk = state.secret.as_ref().map(|key| key.public_key());
    let words = if pk.is_some() { "in memory" } else { "no nsec" };
    let colour = if pk.is_some() { "amber" } else { "teal" };
    b.append(&widgets::identicon(
        pk.as_ref().map(|key| &key.0),
        &format!("{colour}-px"),
        false,
    ));
    if !state.narrow {
        if let Some(pk) = &pk {
            b.append(&line(&pk.short_npub(), &["pill-npub"]));
        }
    }
    b.append(&line(words, &["pill-words", colour]));
    b.update_property(&[gtk::accessible::Property::Label(&format!("Key: {words}"))]);
    b
}

fn logo() -> gtk::Box {
    let b = hbox(2);
    b.set_valign(gtk::Align::Center);
    for (h, c) in [(9, "logo-amber"), (16, "logo-teal"), (12, "logo-amber")] {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("logo-bar");
        bar.add_css_class(c);
        bar.set_size_request(4, h);
        bar.set_valign(gtk::Align::End);
        b.append(&bar);
    }
    b.set_size_request(18, 18);
    b
}

fn stop_dialog(heading: &str, action: &str) -> adw::AlertDialog {
    let d = adw::AlertDialog::new(
        Some(heading),
        Some("The hike ends after the leg in progress. No object is written, and the hike does not keep running after the app exits."),
    );
    d.add_response("cancel", "Keep recording");
    d.add_response("stop", action);
    d.set_response_appearance("stop", adw::ResponseAppearance::Destructive);
    d.set_default_response(Some("cancel"));
    d.set_close_response("cancel");
    d
}
