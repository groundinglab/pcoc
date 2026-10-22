//! Verify. The section 14 record, plus a clear line when the file is not a PCOC.

use crate::state::VerifyState;
use crate::ui::App;
use crate::widgets::{button, card, hbox, learn, line, mono, text, token, vbox};
use gtk::prelude::*;
use pcoc_hike_core::format::{thousands, tilde};
use pcoc_hike_core::record::{Presented, RecordView};
use pcoc_hike_core::verify_job::Outcome;
use std::path::Path;
use std::rc::Rc;

pub fn build(app: &Rc<App>) -> gtk::Widget {
    let page = vbox(12);
    page.set_margin_top(12);
    page.set_margin_bottom(16);
    page.set_margin_start(12);
    page.set_margin_end(12);
    let state = app.state.borrow().verify.clone();
    let on = app.state.borrow().learn();
    match state {
        VerifyState::Empty => page.append(&empty(app, on)),
        VerifyState::IdleError { path, message } => {
            page.append(&idle_error(app, &path, &message, on))
        }
        VerifyState::Selected(path) => page.append(&selected(app, &path, on)),
        VerifyState::Running(path) => {
            let c = card("Measuring", &["§14"], &[], on);
            let row = hbox(10);
            let spinner = gtk::Spinner::new();
            spinner.start();
            row.append(&spinner);
            row.append(&text(
                &format!("{} · reading the portable index", tilde(&path)),
                &["lead"],
            ));
            c.body.append(&row);
            learn(&c.body, &c.root, "Verify does not Observe and does not call Bari. It reads stored headers and plant blocks from the portable index, and runs the six-leg draw.", on);
            page.append(&c.root);
        }
        VerifyState::Done(run) => match &run.outcome {
            Outcome::CouldNotOpen(err) => {
                let c = card("Object", &[], &[], on);
                c.body.append(&text(
                    &format!("Fail. The file could not be opened: {err}."),
                    &["amber", "lead"],
                ));
                c.body.append(&open_buttons(app));
                page.append(&c.root);
            }
            Outcome::Unreadable => {
                let c = card("Object", &[], &[], on);
                c.body.append(&mono(&tilde(&run.path)));
                c.body.append(&text(
                    "Fail. This file is not a PCOC this app can read, so there is no record.",
                    &["amber", "lead"],
                ));
                c.body.append(&open_buttons(app));
                page.append(&c.root);
            }
            Outcome::Record {
                presented: Presented::Unknown { kind_text },
                bytes,
                sha256,
                ..
            } => {
                let c = card(
                    "Object",
                    &[kind_text.as_str()],
                    &[&format!("{} bytes", thousands(*bytes))],
                    on,
                );
                c.body.append(&mono(&tilde(&run.path)));
                c.body.append(&mono(&pcoc_hike_core::hex::encode(sha256)));
                c.body.append(&text(
                    "Fail. This file is an unknown kind (Rule A5). This app checks kind v2.2-pcoc-object.",
                    &["amber", "lead"],
                ));
                let row = hbox(8);
                row.append(&line("kind", &["muted"]));
                row.append(&token("unknown kind"));
                c.body.append(&row);
                learn(&c.body, &c.root, "Rule A5. An unknown kind is a measurement, not a decode failure. The stage matrix is not drawn for it.", on);
                page.append(&c.root);
            }
            Outcome::Record {
                presented: Presented::Record(r),
                bytes: _,
                sha256: _,
                json: _,
            } => {
                page.append(&trimmed_card(r, &run.path, on));
                page.append(&open_buttons(app));
            }
        },
    }
    page.upcast()
}

fn trimmed_card(r: &RecordView, path: &Path, on: bool) -> gtk::Widget {
    let t = r.trimmed();
    let c = card("PCOC", &["§14"], &[], on);
    c.body.append(&mono(&tilde(path)));
    let row = |name: &str, value: &str| {
        let line_box = hbox(12);
        line_box.append(&line(name, &["muted"]));
        let value = line(value, &["strong"]);
        value.set_hexpand(true);
        value.set_halign(gtk::Align::End);
        line_box.append(&value);
        line_box
    };
    c.body.append(&row("Checkpoints", &t.checkpoints));
    c.body.append(&row("Stages", &t.stages));
    c.body.append(&row("Span", &t.span));
    c.body.append(&row("Rate", &t.rate));
    learn(
        &c.body,
        &c.root,
        "Checkpoints are the stored leg results. Stages are the eighteen stage rows. Span is whole_hike_span. Rate is N ÷ S.",
        on,
    );
    c.root.upcast()
}

fn open_buttons(app: &Rc<App>) -> gtk::Box {
    let row = hbox(8);
    let open = button("Open a PCOC…", "outline");
    let app = Rc::clone(app);
    open.connect_clicked(move |_| app.open_object_dialog());
    row.append(&open);
    row
}

fn idle_error(app: &Rc<App>, path: &Path, message: &str, on: bool) -> gtk::Widget {
    let c = card("Object", &[], &[], on);
    c.body.append(&mono(&tilde(path)));
    c.body.append(&text(message, &["lead"]));
    c.body.append(&open_buttons(app));
    learn(
        &c.body,
        &c.root,
        "This file was not measured. Open another PCOC. Verify does not keep the previous measurement.",
        on,
    );
    c.root.upcast()
}

fn empty(app: &Rc<App>, on: bool) -> gtk::Widget {
    let c = card("Verify", &["§14"], &[], on);
    c.body.append(&text(
        "Open a PCOC, or pick a hike from the list.",
        &["lead"],
    ));
    c.body.append(&open_buttons(app));
    learn(
        &c.body,
        &c.root,
        &format!(
            "Verify measures the PCOC with the pinned crate and shows the section 14 record. It runs only when you ask. It reads the portable index at {} and nothing else.",
            pcoc_hike_core::endpoints::INDEX
        ),
        on,
    );
    c.root.upcast()
}

fn selected(app: &Rc<App>, path: &Path, on: bool) -> gtk::Widget {
    let c = card("Object", &[], &[], on);
    c.body.append(&mono(&tilde(path)));
    let row = hbox(8);
    let go = button("Verify", "primary");
    {
        let app = Rc::clone(app);
        let path = path.to_owned();
        go.connect_clicked(move |_| app.start_verify(path.clone()));
    }
    row.append(&go);
    row.append(&open_buttons(app));
    c.body.append(&row);
    let threads = app.state.borrow().config.threads();
    learn(
        &c.body,
        &c.root,
        &format!("Verify runs {threads} checkpoint comparisons at once. The record does not depend on that number. Each verification draws its own six legs."),
        on,
    );
    c.root.upcast()
}
