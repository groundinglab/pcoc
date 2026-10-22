//! Settings and About (DESIGN-v2 section 7.7, US-9). The few real
//! preferences, the locked endpoints, and the pin this build links.

use crate::ui::App;
use crate::widgets::{card, columns, hbox, kv, learn, line, mono, separator, text, vbox};
use gtk::gio;
use gtk::prelude::*;
use pcoc_hike_core::about;
use pcoc_hike_core::config::{Theme, THREADS_MAX, THREADS_MIN};
use pcoc_hike_core::endpoints;
use pcoc_hike_core::format::tilde;
use std::rc::Rc;

pub fn build(app: &Rc<App>) -> gtk::Widget {
    let s = app.state.borrow();
    let on = s.learn();
    let narrow = s.narrow;
    drop(s);

    let page = vbox(14);
    page.set_margin_top(16);
    page.set_margin_bottom(16);
    page.set_margin_start(16);
    page.set_margin_end(16);

    let pair = |a: gtk::Widget, b: gtk::Widget| -> gtk::Widget {
        if narrow {
            let col = vbox(14);
            col.append(&a);
            col.append(&b);
            col.upcast()
        } else {
            columns(&[a, b], 14).upcast()
        }
    };
    page.append(&pair(appearance(app, on), work(app, on)));
    page.append(&pair(network(app, on), measurement(on)));
    page.append(&about_card(on));
    page.upcast()
}

/// A setting: name and explanation on the left, the control on the right.
fn setting(name: &str, explain: &str, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = hbox(12);
    let words = vbox(2);
    words.set_hexpand(true);
    words.append(&line(name, &["strong"]));
    words.append(&text(explain, &["muted", "small"]));
    row.append(&words);
    let control = control.as_ref();
    control.set_valign(gtk::Align::Center);
    row.append(control);
    row
}

fn appearance(app: &Rc<App>, on: bool) -> gtk::Widget {
    let c = card("Appearance", &[], &[], on);
    let theme = app.state.borrow().config.effective_theme();
    let seg = hbox(0);
    seg.add_css_class("seg");
    let dark = gtk::ToggleButton::with_label("Dark");
    let light = gtk::ToggleButton::with_label("Light");
    light.set_group(Some(&dark));
    dark.set_active(theme == Theme::Dark);
    light.set_active(theme == Theme::Light);
    for (b, t) in [(&dark, Theme::Dark), (&light, Theme::Light)] {
        let weak = Rc::downgrade(app);
        b.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            if let Some(app) = weak.upgrade() {
                if app.state.borrow().config.effective_theme() != t {
                    let app = app.clone();
                    gtk::glib::idle_add_local_once(move || app.set_theme(t));
                }
            }
        });
        seg.append(b);
    }
    c.body.append(&setting(
        "Theme",
        "Dark is the default for a new install.",
        &seg,
    ));
    c.root.upcast()
}

fn work(app: &Rc<App>, on: bool) -> gtk::Widget {
    let c = card("Work", &["A4.4.4"], &[], on);
    let (folder, threads, running) = {
        let s = app.state.borrow();
        (
            s.config.data_folder.clone(),
            s.config.threads(),
            s.produce.running(),
        )
    };
    let folder_box = hbox(8);
    folder_box.append(&line(&tilde(&folder), &["mono", "strong"]));
    let choose = gtk::Button::with_label("Choose…");
    choose.add_css_class("outline");
    choose.set_sensitive(!running);
    {
        let weak = Rc::downgrade(app);
        choose.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                choose_folder(&app);
            }
        });
    }
    folder_box.append(&choose);
    c.body.append(&setting(
        "Data folder",
        "Objects and their .events siblings.",
        &folder_box,
    ));
    c.body.append(&separator());

    let spin = gtk::SpinButton::with_range(f64::from(THREADS_MIN), f64::from(THREADS_MAX), 1.0);
    spin.set_value(threads as f64);
    spin.set_digits(0);
    spin.set_numeric(true);
    spin.update_property(&[gtk::accessible::Property::Label("Checkpoint threads")]);
    {
        let weak = Rc::downgrade(app);
        spin.connect_value_changed(move |sp| {
            if let Some(app) = weak.upgrade() {
                let mut s = app.state.borrow_mut();
                s.config.threads = sp.value_as_int() as u32;
                s.save_config();
            }
        });
    }
    c.body.append(&setting(
        "Checkpoint threads",
        &format!(
            "Default is {}. The verify record does not change.",
            pcoc_hike_core::config::Config::default().threads
        ),
        &spin,
    ));
    c.body.append(&separator());
    c.body.append(&crate::pages::produce::test_admission(app));
    c.root.upcast()
}

fn choose_folder(app: &Rc<App>) {
    let dialog = gtk::FileDialog::builder()
        .title("Choose the data folder")
        .modal(true)
        .build();
    let folder = app.state.borrow().config.data_folder.clone();
    if folder.is_dir() {
        dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
    }
    let weak = Rc::downgrade(app);
    dialog.select_folder(Some(&app.window), None::<&gio::Cancellable>, move |res| {
        let (Some(app), Ok(file)) = (weak.upgrade(), res) else {
            return;
        };
        let Some(path) = file.path() else {
            return;
        };
        {
            let mut s = app.state.borrow_mut();
            s.config.data_folder = path;
            s.save_config();
            s.rescan();
        }
        app.refresh_all();
    });
}

fn network(app: &Rc<App>, on: bool) -> gtk::Widget {
    let c = card("Network", &["§9"], &[], on);
    let peer = gtk::Entry::new();
    peer.set_placeholder_text(Some("host:port"));
    peer.set_text(&app.state.borrow().config.gossip_peer);
    peer.add_css_class("mono");
    peer.set_width_chars(18);
    peer.update_property(&[gtk::accessible::Property::Label(
        "Gossip peer override, host:port",
    )]);
    {
        let weak = Rc::downgrade(app);
        peer.connect_changed(move |e| {
            if let Some(app) = weak.upgrade() {
                let mut s = app.state.borrow_mut();
                s.config.gossip_peer = e.text().trim().to_owned();
                s.save_config();
            }
        });
    }
    c.body.append(&setting(
        "Gossip peer override",
        "live header source is not in this build. A named peer is stored and is not dialed.",
        &peer,
    ));
    c.body
        .append(&text(&about::dns_seeds(), &["muted", "small"]));
    let endpoint = |label: &str, value: &str| -> gtk::Box {
        let row = hbox(8);
        let name = line(label, &[]);
        name.set_hexpand(false);
        row.append(&name);
        let value = line(value, &["strong"]);
        value.set_hexpand(true);
        value.set_wrap(true);
        value.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        value.set_xalign(1.0);
        row.append(&value);
        row
    };
    c.body.append(&endpoint("Bitcoin", about::BITCOIN));
    c.body.append(&endpoint("Admission", endpoints::ADMISSION));
    c.body.append(&endpoint("Bari", endpoints::BARI));
    c.body.append(&endpoint("Index", endpoints::INDEX));
    c.body.append(&endpoint("Verify", endpoints::VERIFY));
    c.body.append(&text(
        "These four endpoints are set in one place. Verify reads the index. Sign-in and test admission use the admission gate. The verify host is the public verifier. live header source is not in this build.",
        &["muted", "small"],
    ));
    learn(
        &c.body,
        &c.root,
        "live header source is not in this build. A named peer is remembered for a later header source. This build does not dial it.",
        on,
    );
    c.root.upcast()
}

fn measurement(on: bool) -> gtk::Widget {
    let c = card("Measurement", &[], &[], on);
    c.body.append(&text(
        "Verify shows the section 14 record. A file that is not a PCOC fails in one line. Test admission on Produce is the gate's pass or fail. Auto-verify after produce is off.",
        &["lead"],
    ));
    c.body.append(&text(
        "Stopping a hike, or quitting during one, writes no object. The hike does not keep running after the app exits.",
        &["muted", "small"],
    ));
    c.root.upcast()
}

fn about_card(on: bool) -> gtk::Widget {
    let c = card("About", &[], &[], on);
    let install = if std::path::Path::new("/.flatpak-info").exists() {
        format!("Flatpak · {}", about::APP_ID)
    } else {
        format!(
            "built from source · Flatpak id {} · not published",
            about::APP_ID
        )
    };
    let mut rows: Vec<(&str, gtk::Widget)> = vec![
        ("SPEC", line(about::SPEC, &[]).upcast()),
        ("Hard cut", line(about::SPEC_HARD_CUT, &["mono"]).upcast()),
        ("Sign-in", line(about::SPEC_SIGN_IN_LOCK, &[]).upcast()),
        ("PCOC SDK", line(about::sdk_line(), &[]).upcast()),
        ("SDK source", text(&about::source_line(), &[]).upcast()),
    ];
    if about::shows_resolved_commit() {
        rows.push(("Resolved commit", mono(about::PIN_COMMIT).upcast()));
    }
    rows.extend([
        ("Object kind", line(about::OBJECT_KIND, &["mono"]).upcast()),
        (
            "App",
            line(&format!("PCOC Hike {}", about::APP_VERSION), &[]).upcast(),
        ),
        ("Install", text(&install, &[]).upcast()),
        ("By", line("groundinglab", &[]).upcast()),
    ]);
    c.body.append(&kv(&rows, 140));
    let note = about::pin_note();
    c.body.append(&text(&note, &["muted", "small"]));
    learn(
        &c.body,
        &c.root,
        "Produce progress is read from the SDK's produce closures and its .events log. A dedicated progress callback would be a change inside the SDK, reviewed with the next SDK release.",
        on,
    );
    c.root.upcast()
}
