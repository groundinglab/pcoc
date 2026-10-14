//! Sign-in. Paste a `https://groundinglab.io/sign-in/?v=1` link and an nsec,
//! choose the PCOC, and press once. The nsec stays in memory.

use crate::state::{Page, SignInState};
use crate::ui::App;
use crate::widgets::{button, card, hbox, learn, line, mono, text, vbox};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use pcoc_hike_core::format::{clock, iso, thousands, tilde};
use pcoc_hike_core::signin::{self, Challenge};
use std::rc::Rc;

pub fn build(app: &Rc<App>) -> gtk::Widget {
    let outer = vbox(0);
    outer.set_valign(gtk::Align::Start);
    outer.set_vexpand(true);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);

    let s = app.state.borrow();
    let on = s.learn();
    let state = s.sign_in.clone();
    let typed = s.sign_in_text.clone();
    drop(s);

    let c = card("Sign in", &["§15.1"], &[], on);
    match &state {
        SignInState::Done(words) => {
            let tone = if words.starts_with("Pass") {
                "teal"
            } else if words.starts_with("Fail") {
                "amber"
            } else {
                ""
            };
            let l = text(words, &[tone, "lead"]);
            l.set_selectable(true);
            c.body.append(&l);
            c.body.append(&again_button(app));
        }
        SignInState::Sending => {
            let row = hbox(10);
            let spinner = gtk::Spinner::new();
            spinner.start();
            row.append(&spinner);
            row.append(&line("Submitting once to the admission gate.", &["muted"]));
            c.body.append(&row);
        }
        SignInState::Empty | SignInState::Bad(_) | SignInState::Parsed(_) => {
            c.body.append(&text(
                "Paste the link from the sign-in page and the nsec that owns the PCOC. Press Sign in to sign it once.",
                &["lead"],
            ));
            c.body.append(&open_sign_in_page(app));
            c.body.append(&line("LINK", &["card-title"]));
            c.body.append(&link_box(app, &state, &typed));
            match &state {
                SignInState::Bad(why) => c.body.append(&text(why.sentence(), &["amber"])),
                SignInState::Parsed(challenge) => {
                    challenge_view(app, &c.body, challenge, on);
                    c.body.append(&nsec_block(app));
                    c.body.append(&object_block(app));
                    c.body.append(&sign_button(app));
                }
                SignInState::Empty => {
                    c.body.append(&nsec_block(app));
                    learn(
                        &c.body,
                        &c.root,
                        "The link carries a nonce and an expiry. It does not name a PCOC. A link with an object parameter is refused.",
                        on,
                    );
                }
                _ => {}
            }
        }
    }
    let clamp = adw::Clamp::builder()
        .maximum_size(760)
        .tightening_threshold(600)
        .child(&c.root)
        .build();
    clamp.set_hexpand(true);
    outer.append(&clamp);
    outer.upcast()
}

fn link_box(app: &Rc<App>, state: &SignInState, typed: &str) -> gtk::Widget {
    let view = gtk::TextView::new();
    view.add_css_class("mono");
    view.add_css_class("link-box");
    view.set_wrap_mode(gtk::WrapMode::Char);
    view.set_accepts_tab(false);
    view.set_top_margin(4);
    view.set_bottom_margin(4);
    view.set_size_request(-1, 64);
    view.update_property(&[gtk::accessible::Property::Label("Sign-in link")]);
    match state {
        SignInState::Parsed(challenge) => {
            view.buffer().set_text(&challenge.link_lines());
            view.set_editable(false);
            view.set_cursor_visible(false);
            let b = vbox(6);
            b.append(&view);
            b.append(&again_button(app));
            return b.upcast();
        }
        _ => view.buffer().set_text(typed),
    }
    let weak = Rc::downgrade(app);
    view.buffer().connect_changed(move |buffer| {
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        let weak = weak.clone();
        glib::idle_add_local_once(move || {
            if let Some(app) = weak.upgrade() {
                app.set_sign_in_text(text);
            }
        });
    });
    view.upcast()
}

fn open_sign_in_page(app: &Rc<App>) -> gtk::Button {
    let b = button("Open sign-in page", "outline");
    b.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(app);
    b.connect_clicked(move |_| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        gtk::UriLauncher::new("https://groundinglab.io/sign-in/").launch(
            Some(&app.window),
            None::<&gio::Cancellable>,
            |_| {},
        );
    });
    b
}

fn again_button(app: &Rc<App>) -> gtk::Button {
    let b = button("Paste another link", "outline");
    b.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(app);
    b.connect_clicked(move |_| {
        if let Some(app) = weak.upgrade() {
            {
                let mut s = app.state.borrow_mut();
                s.sign_in = SignInState::Empty;
                s.sign_in_text.clear();
            }
            app.refresh_all();
        }
    });
    b
}

fn nsec_block(app: &Rc<App>) -> gtk::Widget {
    let block = vbox(6);
    block.append(&line("NSEC", &["card-title"]));
    let row = hbox(8);
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_width_chars(20);
    entry.add_css_class("mono");
    entry.set_placeholder_text(Some("your private key (starts with nsec)"));
    entry.update_property(&[gtk::accessible::Property::Label("nsec")]);
    let current = app
        .state
        .borrow()
        .secret
        .as_ref()
        .map(|key| key.to_nsec().to_string())
        .unwrap_or_default();
    entry.set_text(&current);
    let npub = mono("—");
    let hex = mono("—");
    let err = text("", &["amber"]);
    err.set_visible(false);
    if let Some(secret) = app.state.borrow().secret.as_ref() {
        let public = secret.public_key();
        npub.set_text(&public.npub());
        hex.set_text(&public.hex());
    }
    {
        let app = Rc::clone(app);
        let npub = npub.clone();
        let hex = hex.clone();
        let err = err.clone();
        entry.connect_changed(move |entry| {
            app.show_nsec(entry.text().as_str(), &npub, &hex, &err);
        });
    }
    row.append(&entry);
    block.append(&row);
    block.append(&err);
    block.append(&text(
        "Save this nsec, npub, and hex somewhere else. This app keeps the nsec in memory only and never writes it.",
        &["amber", "small"],
    ));
    block.append(&line("NPUB", &["card-title"]));
    block.append(&npub);
    block.append(&line("HEX", &["card-title"]));
    block.append(&hex);
    block.upcast()
}

fn object_block(app: &Rc<App>) -> gtk::Widget {
    let block = vbox(6);
    block.append(&line("PCOC", &["card-title"]));
    let path = {
        let mut s = app.state.borrow_mut();
        if s.sign_in_path.is_none() {
            let produced = s.produce.object.as_ref().map(|object| object.path.clone());
            s.sign_in_path =
                pcoc_hike_core::library::session_prefill(s.secret.is_some(), produced).pcoc;
        }
        s.sign_in_path.clone()
    };
    let shown = path
        .as_ref()
        .map(|p| tilde(p))
        .unwrap_or_else(|| "No PCOC chosen.".to_owned());
    block.append(&mono(&shown));
    let choose = button("Choose a PCOC…", "outline");
    choose.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(app);
    choose.connect_clicked(move |_| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let dialog = gtk::FileDialog::builder()
            .title("PCOC to sign")
            .modal(true)
            .build();
        let folder = app.state.borrow().config.data_folder.clone();
        if folder.is_dir() {
            dialog.set_initial_folder(Some(&gio::File::for_path(&folder)));
        }
        let weak = Rc::downgrade(&app);
        dialog.open(Some(&app.window), None::<&gio::Cancellable>, move |res| {
            if let (Some(app), Ok(file)) = (weak.upgrade(), res) {
                if let Some(path) = file.path() {
                    app.state.borrow_mut().sign_in_path = Some(path);
                    app.refresh_all();
                }
            }
        });
    });
    block.append(&choose);
    block.upcast()
}

fn sign_button(app: &Rc<App>) -> gtk::Widget {
    let b = button("Sign in", "primary");
    b.set_halign(gtk::Align::Start);
    let weak = Rc::downgrade(app);
    b.connect_clicked(move |_| {
        if let Some(app) = weak.upgrade() {
            app.sign_and_submit();
        }
    });
    b.upcast()
}

fn expiry_words(challenge: &Challenge, now_s: u64) -> String {
    let left = challenge.seconds_left(now_s);
    let secs = thousands(left.unsigned_abs().min(u128::from(u64::MAX)) as u64);
    let span = if left >= 0 {
        format!("{secs} s left")
    } else {
        format!("{secs} s past the expiry")
    };
    if left.unsigned_abs() >= 86_400 {
        return format!(
            "{} · {span} · now {}",
            iso(challenge.expiry_unix_s),
            iso(now_s)
        );
    }
    format!(
        "{} UTC · {span} · now {} UTC",
        clock(challenge.expiry_unix_s),
        clock(now_s)
    )
}

fn challenge_view(app: &Rc<App>, body: &gtk::Box, challenge: &Challenge, on: bool) {
    body.append(&line("Requesting origin", &["muted", "small"]));
    let origin = line(signin::ORIGIN, &["origin"]);
    origin.set_selectable(true);
    body.append(&origin);
    let expiry = line(&expiry_words(challenge, app.state.borrow().now_s()), &[]);
    {
        let expiry = expiry.clone();
        let challenge = challenge.clone();
        app.tick(Page::SignIn, move |state| {
            expiry.set_text(&expiry_words(&challenge, state.now_s()));
        });
    }
    body.append(&expiry);
    learn(
        body,
        body,
        "Sign in posts the link, this nsec's signature, and the PCOC once. The gate answers pass or fail.",
        on,
    );
}
