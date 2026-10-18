//! Produce (DESIGN-v2 sections 7.3, 7.4 and 7.8). The unit is the leg.
//! No step counter, no percentage. Time left is an estimate from headers
//! still to come.

use crate::state::{Page, State};
use crate::ui::App;
use crate::widgets::{self, button, card, hbox, learn, line, mono, text, vbox};
use gtk::prelude::*;
use pcoc_hike_core::endpoints;
use pcoc_hike_core::format::{clock, clock_hm, duration_hm, duration_hms, thousands, tilde};
use pcoc_hike_core::produce::cells::{Cell, Row};
use pcoc_hike_core::produce::events::AskOutcome;
use pcoc_hike_core::produce::model::{Phase, ProduceView, WaypointChip, READING_THE_TIP};
use pcoc_hike_core::produce::{STAGES, STEPS_PER_LEG, TERRAIN_BYTES};
use std::rc::Rc;

pub fn build(app: &Rc<App>) -> gtk::Widget {
    let page = vbox(12);
    page.set_margin_top(12);
    page.set_margin_bottom(16);
    page.set_margin_start(12);
    page.set_margin_end(12);
    let (phase, result, failed) = {
        let s = app.state.borrow();
        let failed = match &s.produce.phase {
            Phase::Failed(message) => Some(message.clone()),
            _ => None,
        };
        (s.produce.phase.clone(), s.result_path.clone(), failed)
    };
    if let Some(path) = result {
        let session = app
            .state
            .borrow()
            .produce
            .object
            .as_ref()
            .map(|object| object.path.clone());
        page.set_widget_name("produce-result");
        if session.as_ref() == Some(&path) && phase == Phase::Written {
            page.append(&actions(app, &path));
            page.append(&written_body(app));
        } else {
            page.append(&compact_result(app, &path));
        }
        return page.upcast();
    }
    match phase {
        Phase::Idle => {
            page.set_widget_name("produce-setup");
            page.append(&idle(app));
        }
        Phase::Stopped => {
            page.set_widget_name("produce-setup");
            page.append(&idle(app));
        }
        Phase::Failed(_) => {
            page.set_widget_name("produce-setup");
            if let Some(message) = failed {
                page.append(&text(
                    &format!("The hike stopped: {message}. No object was written."),
                    &["lead"],
                ));
            }
            page.append(&idle(app));
        }
        Phase::Written => {
            page.set_widget_name("produce-result");
            if let Some(path) = app
                .state
                .borrow()
                .produce
                .object
                .as_ref()
                .map(|object| object.path.clone())
            {
                page.append(&actions(app, &path));
            }
            page.append(&written_body(app));
        }
        Phase::Starting | Phase::Recording | Phase::Settling => {
            page.set_widget_name("produce-live");
            page.append(&live_body(app));
        }
    }
    page.upcast()
}

fn live_body(app: &Rc<App>) -> gtk::Widget {
    let body = vbox(12);
    body.append(&hero(app));
    body.append(&stage_chart(app));
    body.append(&waypoints(app));
    body.append(&gossip_panel(app));
    body.append(&log_drawer(app));
    body.upcast()
}

fn written_body(app: &Rc<App>) -> gtk::Widget {
    let body = vbox(12);
    body.append(&hero(app));
    body.append(&stage_chart(app));
    body.append(&waypoints(app));
    body.append(&log_drawer(app));
    body.upcast()
}

fn actions(app: &Rc<App>, path: &std::path::Path) -> gtk::Widget {
    let row = hbox(8);
    let verify = button("Verify this PCOC", "primary");
    {
        let app = Rc::clone(app);
        let path = path.to_owned();
        verify.connect_clicked(move |_| app.verify_pcoc(path.clone()));
    }
    row.append(&verify);
    let sign = button("Sign in with it", "outline");
    {
        let app = Rc::clone(app);
        let path = path.to_owned();
        sign.connect_clicked(move |_| app.sign_in_with(path.clone()));
    }
    let (note, busy) = {
        let s = app.state.borrow();
        (s.phone_note.clone(), s.phone_busy)
    };
    let phone = button(
        if busy {
            "Sending…"
        } else {
            "Send to my phone"
        },
        "outline",
    );
    phone.set_sensitive(!busy);
    phone.set_tooltip_text(Some(
        "Send this PCOC to its own npub as an encrypted Nostr message, for the phone app signed in with that npub. Making a PCOC never sends it by itself.",
    ));
    {
        let app = Rc::clone(app);
        let path = path.to_owned();
        phone.connect_clicked(move |_| app.send_to_phone(path.clone()));
    }
    row.append(&phone);
    row.append(&sign);
    let Some(note) = note else {
        return row.upcast();
    };
    let block = vbox(6);
    block.append(&row);
    let tone = if note.starts_with("Sent") {
        "teal"
    } else if note.starts_with("Not sent") {
        "amber"
    } else {
        "muted"
    };
    let label = text(&note, &[tone, "small"]);
    label.set_selectable(true);
    block.append(&label);
    block.upcast()
}

fn compact_result(app: &Rc<App>, path: &std::path::Path) -> gtk::Widget {
    let c = card("Hike", &[], &["finished"], app.state.borrow().learn());
    let name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    c.body.append(&line(&name, &["hero"]));
    c.body.append(&line("finished", &["teal", "strong"]));
    c.body.append(&mono(&pcoc_hike_core::format::tilde(path)));
    c.body.append(&actions(app, path));
    c.root.upcast()
}

fn idle(app: &Rc<App>) -> gtk::Widget {
    let s = app.state.borrow();
    let on = s.learn();
    let key_error = s.key_error.clone();
    drop(s);

    let c = card(
        "New hike",
        &["§13"],
        &["Bitcoin mainnet only · posts to the public Bari · no test plane"],
        on,
    );
    c.body.append(&nsec_block(app, key_error.as_deref()));
    let start = button("Start hike", "primary");
    start.set_halign(gtk::Align::Start);
    {
        let app = Rc::clone(app);
        start.connect_clicked(move |_| app.start_hike());
    }
    c.body.append(&start);
    learn(
        &c.body,
        &c.root,
        &format!(
            "The pinned hike crate runs in this process on Bitcoin mainnet only. The app has no test plane. It holds {} MiB of terrain for the run, posts each waypoint W_1…W_17 to {} with a Hashcash planting puzzle, and asks for paths at H+2, H+3 and H+4. Each run is a new folder under the data folder. The object is written only after the run returns. Stop or quit writes no object.",
            TERRAIN_BYTES >> 20,
            endpoints::BARI
        ),
        on,
    );
    c.root.upcast()
}

fn nsec_block(app: &Rc<App>, key_error: Option<&str>) -> gtk::Widget {
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
    if let Some(message) = key_error {
        err.set_text(message);
        err.set_visible(true);
    } else {
        err.set_visible(false);
    }
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
    let generate = button("Generate", "outline");
    {
        let entry = entry.clone();
        generate.connect_clicked(move |_| {
            let secret = pcoc_hike_core::keys::SecretKey::generate();
            entry.set_text(secret.to_nsec().as_str());
        });
    }
    row.append(&entry);
    row.append(&generate);
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

pub(crate) fn test_admission(app: &Rc<App>) -> gtk::Widget {
    let block = vbox(6);
    block.append(&line("TEST ADMISSION", &["card-title"]));
    block.append(&text(
        "Submit a PCOC to the gate with this nsec. The hike just written is used, or the newest one in the list.",
        &["small"],
    ));
    let (note, busy) = {
        let s = app.state.borrow();
        (s.test_note.clone(), s.test_busy)
    };
    if let Some(note) = note {
        let tone = if note.starts_with("Pass") {
            "teal"
        } else if note.starts_with("Fail") {
            "amber"
        } else {
            "muted"
        };
        let label = text(&note, &[tone]);
        label.set_selectable(true);
        block.append(&label);
    }
    let b = button(
        if busy {
            "Submitting…"
        } else {
            "Test admission"
        },
        "outline",
    );
    b.set_halign(gtk::Align::Start);
    b.set_sensitive(!busy);
    let app = Rc::clone(app);
    b.connect_clicked(move |_| app.test_admission());
    block.append(&b);
    block.upcast()
}

fn hero_card(title: &str, section: &str, note: &[&str], on: bool) -> widgets::Card {
    let c = card(title, &[section], note, on);
    c.root.set_valign(gtk::Align::Fill);
    c
}

fn hero(app: &Rc<App>) -> gtk::Widget {
    let s = app.state.borrow();
    let on = s.learn();
    let p = &s.produce;
    let now = s.now_ms();
    let mut cards: Vec<gtk::Widget> = Vec::new();
    if p.phase == Phase::Written {
        let c = hero_card("Stages", "§13.2", &[], on);
        c.body.append(&line(
            &format!("{} of {STAGES}", p.stages_recorded()),
            &["hero"],
        ));
        c.body.append(&line("stages recorded", &["lead"]));
        learn(&c.body, &c.root, "Stages are numbered 0–17. All eighteen are stored. The object exists only because the hike returned.", on);
        cards.push(c.root.upcast());

        let c = hero_card("Legs", "§11", &[], on);
        c.body.append(&line(&thousands(p.legs_total), &["hero"]));
        c.body.append(&line("checkpoints stored", &["lead"]));
        learn(&c.body, &c.root, &format!("The stage counts below sum to {}. This screen does not show a rate. Ask time is not in the mid-run rate.", thousands(p.legs_total)), on);
        cards.push(c.root.upcast());

        let c = hero_card("Wall clock", "", &[], on);
        let (wall, from, to) = match (p.wall_s(), p.started_ms, p.object.as_ref()) {
            (Some(w), Some(st), Some(o)) => {
                (duration_hms(w), clock(st / 1000), clock(o.at_ms / 1000))
            }
            _ => ("—".into(), String::new(), String::new()),
        };
        c.body.append(&line(&wall, &["hero"]));
        c.body
            .append(&line(&format!("{from} – {to} UTC"), &["lead"]));
        learn(&c.body, &c.root, "This is the app's clock, from start to the object write. It is not an nTime span and it is not an estimate.", on);
        cards.push(c.root.upcast());

        let c = hero_card("Object", "§8", &[], on);
        let o = p.object.as_ref();
        c.body.append(&line(
            &o.map(|o| thousands(o.bytes)).unwrap_or_default(),
            &["hero"],
        ));
        c.body.append(&line("bytes · terrain released", &["lead"]));
        let sha = o
            .map(|o| pcoc_hike_core::hex::encode(&o.sha256))
            .unwrap_or_default();
        learn(
            &c.body,
            &c.root,
            &format!(
                "{} MiB was held for the run and has been released. {}",
                TERRAIN_BYTES >> 20,
                pcoc_hike_core::hex::short(&sha)
            ),
            on,
        );
        cards.push(c.root.upcast());
    } else {
        let (_, leg, recorded) = p.hero_stage();
        let title = if p.phase == Phase::Starting || p.seed_height.is_none() {
            "Produce"
        } else if p.legs_total == 0 {
            "Started"
        } else if p.phase == Phase::Settling {
            "Settling"
        } else {
            "Recording"
        };
        let c = hero_card(title, "§13.2", &[], on);
        let hero_text = hero_primary(p);
        let hero_line = line(&hero_text, &["hero"]);
        hero_line.set_wrap(true);
        c.body.append(&hero_line);
        let secondary = line("", &["teal", "strong"]);
        let tertiary = line("", &["strong"]);
        if p.phase == Phase::Starting || p.seed_height.is_none() {
            // The connecting sentence is the only line until the first header.
        } else if p.legs_total == 0 {
            learn(&c.body, &c.root, "The seed beacon is that header. Stage 0’s legs start after the terrain fill. A later header ends the stage. It does not replace the seed.", on);
        } else if p.phase == Phase::Settling {
            secondary.set_text("asking the last paths");
            tertiary.set_text(&recorded);
            learn(&c.body, &c.root, "Every leg is stored. The app waits for headers until the remaining asks are made, then writes the path record and the object.", on);
        } else {
            secondary.set_text(&leg);
            tertiary.set_text(&recorded);
            let g = p.open_stage;
            let in_stage = p.stages[g].legs.unwrap_or(0);
            let done = if g == 0 {
                "No stage is recorded yet.".to_owned()
            } else if g == 1 {
                "Stage 0 is recorded.".to_owned()
            } else {
                format!("Stages 0–{} are recorded.", g - 1)
            };
            learn(
                &c.body,
                &c.root,
                &format!(
                    "Stage {g} is the {} stage. Stages are numbered 0–17. {done} Leg {} is j = {in_stage} and is not stored.",
                    ordinal(g + 1),
                    in_stage + 1
                ),
                on,
            );
        }
        secondary.set_visible(!secondary.text().is_empty());
        tertiary.set_visible(!tertiary.text().is_empty());
        c.body.append(&secondary);
        c.body.append(&tertiary);
        app.tick(Page::Produce, {
            let hero_line = hero_line.clone();
            let secondary = secondary.clone();
            let tertiary = tertiary.clone();
            move |s: &State| {
                let p = &s.produce;
                hero_line.set_text(&hero_primary(p));
                let (second, third) = hero_rest(p);
                secondary.set_text(&second);
                tertiary.set_text(&third);
                secondary.set_visible(!second.is_empty());
                tertiary.set_visible(!third.is_empty());
            }
        });
        cards.push(c.root.upcast());

        let c = hero_card("Legs recorded", "§11", &[], on);
        let legs_line = line(&thousands(p.legs_total), &["hero"]);
        c.body.append(&legs_line);
        match p.rate() {
            Some((rate, span)) => {
                let rate_line = line(&format!("{rate} legs/s"), &["strong"]);
                c.body.append(&rate_line);
                learn(
                    &c.body,
                    &c.root,
                    &format!(
                        "{} ÷ {} s. Terrain fill is not in the rate. A leg is {} steps; this screen counts legs. The rate is not a quota.",
                        thousands(p.legs_total),
                        thousands(span),
                        thousands(STEPS_PER_LEG)
                    ),
                    on,
                );
                app.tick(Page::Produce, move |s: &State| {
                    legs_line.set_text(&thousands(s.produce.legs_total));
                    if let Some((rate, _)) = s.produce.rate() {
                        rate_line.set_text(&format!("{rate} legs/s"));
                    }
                });
            }
            None => {
                c.body.append(&line("rate after two legs", &["muted"]));
                learn(
                    &c.body,
                    &c.root,
                    &format!(
                        "A leg is {} steps; this screen counts legs.",
                        thousands(STEPS_PER_LEG)
                    ),
                    on,
                );
                app.tick(Page::Produce, move |s: &State| {
                    legs_line.set_text(&thousands(s.produce.legs_total));
                });
            }
        }
        cards.push(c.root.upcast());

        let c = hero_card("Time left", "estimate", &[], on);
        match p.estimate(now) {
            Some(e) => {
                let big = line(&duration_hm(e.point_s), &["hero"]);
                c.body.append(&big);
                let range = text(
                    &format!(
                        "most finishes fall between {} and {}",
                        duration_hm(e.low_s),
                        duration_hm(e.high_s)
                    ),
                    &["lead"],
                );
                c.body.append(&range);
                let first = 18 - e.headers_to_come as usize;
                learn(
                    &c.body,
                    &c.root,
                    &format!(
                        "{} headers still to come, I_{first} through I_17. The estimate does not use the leg rate. Finishes ≈ {} UTC.",
                        e.headers_to_come,
                        clock_hm(e.finish_unix_s(now / 1000))
                    ),
                    on,
                );
                app.tick(Page::Produce, move |s: &State| {
                    if let Some(e) = s.produce.estimate(s.now_ms()) {
                        big.set_text(&duration_hm(e.point_s));
                    }
                });
            }
            None => {
                c.body.append(&line("—", &["hero"]));
                c.body.append(&line("after the first header", &["lead"]));
            }
        }
        cards.push(c.root.upcast());

        let c = hero_card("Terrain held", "§6", &[], on);
        let held = line(&format!("{} MiB", TERRAIN_BYTES >> 20), &["hero"]);
        c.body.append(&held);
        c.body.append(&line("for the run duration", &["lead"]));
        app.tick(Page::Produce, move |_s: &State| {
            held.set_text(&format!("{} MiB", TERRAIN_BYTES >> 20));
        });
        learn(
            &c.body,
            &c.root,
            &format!("{} bytes. The SPEC calls this the cost that does not divide. Released when the hike ends.", thousands(TERRAIN_BYTES)),
            on,
        );
        cards.push(c.root.upcast());
    }
    widgets::columns(&cards, 10).upcast()
}

fn hero_primary(p: &ProduceView) -> String {
    if p.phase == Phase::Starting || p.seed_height.is_none() {
        return READING_THE_TIP.to_owned();
    }
    if p.legs_total == 0 {
        return p
            .seed_height
            .map(|height| format!("Seed height {}", thousands(height)))
            .unwrap_or_else(|| READING_THE_TIP.to_owned());
    }
    if p.phase == Phase::Settling {
        return "All legs stored".to_owned();
    }
    p.stage_line()
}

fn hero_rest(p: &ProduceView) -> (String, String) {
    if p.phase == Phase::Starting || p.seed_height.is_none() || p.legs_total == 0 {
        return (String::new(), String::new());
    }
    let (_, leg, recorded) = p.hero_stage();
    if p.phase == Phase::Settling {
        return ("asking the last paths".to_owned(), recorded);
    }
    (leg, recorded)
}

fn ordinal(n: usize) -> String {
    const WORDS: [&str; 18] = [
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
    ];
    WORDS
        .get(n.wrapping_sub(1))
        .map(|w| w.to_string())
        .unwrap_or_else(|| format!("{n}th"))
}

fn stage_chart(app: &Rc<App>) -> gtk::Widget {
    let s = app.state.borrow();
    let on = s.learn();
    let p = &s.produce;
    let written = p.phase == Phase::Written;
    let note = if written {
        format!("counts sum to {}", thousands(p.legs_total))
    } else {
        "18 stages, numbered 0–17".to_owned()
    };
    let c = card("Stages", &[], &[&note], on);
    let max = p
        .stages
        .iter()
        .filter_map(|s| s.legs)
        .max()
        .unwrap_or(1)
        .max(1);
    let chart = gtk::FlowBox::new();
    chart.set_selection_mode(gtk::SelectionMode::None);
    chart.set_homogeneous(true);
    chart.set_min_children_per_line(1);
    chart.set_max_children_per_line(18);
    chart.set_column_spacing(4);
    chart.set_row_spacing(6);
    chart.set_hexpand(true);
    chart.set_margin_top(8);
    let mut counts: Vec<(usize, gtk::Label)> = Vec::new();
    const BAR_H: i32 = 92;
    for (g, col) in p.stages.iter().enumerate() {
        let column = vbox(3);
        column.set_hexpand(true);
        let area = vbox(0);
        area.set_size_request(-1, BAR_H);
        area.set_valign(gtk::Align::End);
        let words = match col.legs {
            Some(n) if n > 0 || col.interrupt_height.is_some() => {
                let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
                bar.add_css_class("bar");
                if col.open {
                    bar.add_css_class("bar-open");
                }
                let h = ((n as f64 / max as f64) * BAR_H as f64).round().max(2.0) as i32;
                bar.set_size_request(-1, h);
                bar.set_hexpand(true);
                bar.set_halign(gtk::Align::Center);
                bar.set_valign(gtk::Align::End);
                bar.set_vexpand(true);
                area.append(&bar);
                column.append(&area);
                let count = line(&thousands(n), &["bar-count"]);
                counts.push((g, count.clone()));
                column.append(&count);
                if col.open {
                    format!("stage {g}, {n} legs stored, in progress")
                } else {
                    format!("stage {g}, {n} legs")
                }
            }
            Some(_) => {
                column.append(&area);
                let count = line("0", &["bar-count"]);
                counts.push((g, count.clone()));
                column.append(&count);
                format!("stage {g}, in progress, no leg stored yet")
            }
            None => {
                column.append(&area);
                column.append(&line("—", &["bar-count", "muted"]));
                format!("stage {g}, not started")
            }
        };
        let num = line(&g.to_string(), &["bar-stage"]);
        column.append(&num);
        let marker = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        marker.set_halign(gtk::Align::Center);
        marker.add_css_class(if col.interrupt_height.is_some() {
            "int-stored"
        } else {
            "int-not"
        });
        column.append(&marker);
        let mut name = words;
        if let Some(h) = col.interrupt_height {
            name.push_str(&format!(", interrupt I_{g} at {}", thousands(h)));
        }
        column.set_tooltip_text(Some(&name.replace('_', "")));
        column.update_property(&[gtk::accessible::Property::Label(&name)]);
        chart.insert(&column, -1);
    }
    if p.running() {
        app.tick(Page::Produce, move |s: &State| {
            for (g, label) in &counts {
                let text = match s.produce.stages.get(*g).and_then(|col| col.legs) {
                    Some(n) => thousands(n),
                    None => "—".to_owned(),
                };
                label.set_text(&text);
            }
        });
    }
    c.body.append(&chart);

    let item = |swatch: &str, words: &str| {
        let b = hbox(5);
        let sw = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        sw.add_css_class("legend-swatch");
        sw.add_css_class(swatch);
        sw.set_valign(gtk::Align::Center);
        b.append(&sw);
        b.append(&text(words, &["muted", "small"]));
        b.upcast()
    };
    let last_int = p
        .stages
        .iter()
        .enumerate()
        .rfind(|(_, s)| s.interrupt_height.is_some())
        .map(|(g, _)| g);
    let mut legend_items: Vec<gtk::Widget> = Vec::new();
    if written {
        legend_items.push(item("sw-teal", "legs in the stage"));
        legend_items.push(item("int-stored", "interrupt I_0 through I_17"));
    } else {
        legend_items.push(item("sw-teal", "legs stored"));
        if p.running() {
            let g = p.open_stage;
            legend_items.push(item(
                "sw-open",
                &format!("stage {g}, {} stored", p.stages[g].legs.unwrap_or(0)),
            ));
        }
        legend_items.push(item(
            "int-stored",
            &match last_int {
                Some(0) => "interrupt stored, I_0".to_owned(),
                Some(g) => format!("interrupt stored, I_0–I_{g}"),
                None => "interrupt stored".to_owned(),
            },
        ));
        legend_items.push(item("int-not", "interrupt not yet"));
    }
    c.body.append(&widgets::wrap_row(&legend_items, 8));

    let mut note = String::new();
    if written {
        if let Some((g, _)) = p.stages.iter().enumerate().find(|(_, s)| s.legs == Some(1)) {
            note.push_str(&format!("Stage {g} recorded 1 leg. The first Observe after that checkpoint was already the interrupt. The leg was not cut short. "));
        }
        if let (Some(cut), Some(seed)) = (p.cut_off(), p.seed_height) {
            note.push_str(&format!(
                "I_17 is the cut-off at {}. Seed beacon {}.",
                thousands(cut),
                thousands(seed)
            ));
        }
    } else if p.running() {
        let g = p.open_stage;
        note.push_str(&format!(
            "Bar height is the leg count. Stage {g} shows {}, not the in-progress leg.",
            p.stages[g].legs.unwrap_or(0)
        ));
        if let Some(seed) = p.seed_height {
            note.push_str(&format!(" Seed beacon {}.", thousands(seed)));
        }
        if let Some(g) = last_int {
            note.push_str(&format!(
                " Latest interrupt I_{g} at {}.",
                thousands(p.stages[g].interrupt_height.unwrap_or(0))
            ));
        }
        note.push_str(" A stage ends at the first Observe after a checkpoint.");
    }
    if !note.is_empty() {
        learn(&c.body, &c.root, &note, on);
    }
    c.root.upcast()
}

fn waypoints(app: &Rc<App>) -> gtk::Widget {
    let s = app.state.borrow();
    let on = s.learn();
    let p = &s.produce;
    let written = p.phase == Phase::Written;
    let c = card(
        "Waypoints",
        &[if written { "§13.1 · A3.4" } else { "§13.1" }],
        &[],
        on,
    );
    let bits = pcoc_hike_core::about::hashcash_bits();
    let summary = if written {
        format!("Hashcash {bits} bits · {}", p.post_summary())
    } else {
        format!(
            "Hashcash {bits} bits · planting puzzle, not the hike's cost · {}",
            p.post_summary()
        )
    };
    let hc = line(&summary, &["violet", "strong", "small"]);
    hc.set_wrap(true);
    hc.set_tooltip_text(Some("Hashcash is the planting puzzle drawn next to waypoints. It buys the plant and does not buy the leg."));
    if p.running() {
        app.tick(Page::Produce, {
            let hc = hc.clone();
            move |s: &State| {
                let bits = pcoc_hike_core::about::hashcash_bits();
                let summary = if written {
                    format!("Hashcash {bits} bits · {}", s.produce.post_summary())
                } else {
                    format!(
                        "Hashcash {bits} bits · planting puzzle, not the hike's cost · {}",
                        s.produce.post_summary()
                    )
                };
                hc.set_text(&summary);
            }
        });
    }
    if s.narrow {
        c.body.append(&hc);
    } else {
        c.head.append(&widgets::spacer());
        c.head.append(&hc);
    }

    let last = if written {
        16
    } else {
        p.waypoints.keys().copied().max().unwrap_or(0).min(16)
    };
    let groups = gtk::FlowBox::new();
    groups.set_selection_mode(gtk::SelectionMode::None);
    groups.set_homogeneous(false);
    groups.set_max_children_per_line(8);
    groups.set_min_children_per_line(1);
    groups.set_column_spacing(6);
    groups.set_row_spacing(8);
    let mut chips: Vec<(u64, gtk::Label)> = Vec::new();
    for g in 1..=last {
        let chip = waypoint_chip_widget(g, p.waypoint_chip(g));
        chips.push((g, chip.clone()));
        groups.insert(&chip, -1);
    }
    if last == 0 {
        c.body.append(&text(
            "Stage 0 is not posted. W_1 is posted when stage 1 opens.",
            &["muted"],
        ));
    } else {
        c.body.append(&groups);
        c.body.append(&waypoint_chip_legend());
        app.tick(Page::Produce, move |s: &State| {
            for (g, chip) in &chips {
                paint_waypoint_chip(chip, s.produce.waypoint_chip(*g));
            }
        });
    }

    if written {
        let block = vbox(2);
        block.add_css_class("code-block");
        for l in &p.path_lines {
            block.append(&line(l, &["mono"]));
        }
        if !p.path_lines.is_empty() {
            c.body.append(&block);
        }
        let mut note = String::from("Those lines are the final stderr step, in that order, after the cut-off. W_17 is posted and never asked.");
        for (g, w) in &p.waypoints {
            if !w.miss_phrases.is_empty() {
                let n = w.miss_phrases.len();
                note.push_str(&format!(
                    " W_{g} missed {}: {}.",
                    if n == 1 {
                        "once".to_owned()
                    } else {
                        format!("{} times", words(n))
                    },
                    w.miss_phrases.join(", ")
                ));
            }
        }
        if let Some(cut) = p.cut_off() {
            if let Some((g, _)) = p.waypoints.iter().find(|(g, w)| {
                w.asks.len() == 1 && p.beacon_height(**g as usize).is_some_and(|h| h + 2 == cut)
            }) {
                note.push_str(&format!(" W_{g} is asked once, at the cut-off."));
            }
        }
        learn(&c.body, &c.root, &note, on);
        if let Some(o) = &p.object {
            c.body.append(&mono(&tilde(&o.path)));
            c.body
                .append(&mono(&pcoc_hike_core::hex::encode(&o.sha256)));
        }
    } else {
        let mut note = String::new();
        for g in p.not_connected() {
            note.push_str(&format!(
                "W_{g}'s POST did not connect, so it has no asks. "
            ));
        }
        let mut served: Vec<(u64, u64)> = Vec::new();
        for (g, w) in &p.waypoints {
            for (offset, outcome) in &w.asks {
                if *outcome == AskOutcome::Served {
                    if let Some(h) = p.beacon_height(*g as usize) {
                        served.push((*g, h + offset));
                    }
                }
            }
        }
        if let Some((g, h)) = served.iter().max_by_key(|(_, h)| *h) {
            note.push_str(&format!("W_{g} was served at {}. ", thousands(*h)));
        }
        for g in 1..=last {
            if let Row::Cells(cs) = p.waypoint_row(g) {
                if let Some(i) = cs.iter().position(|c| *c == Cell::StillDue) {
                    let h = p
                        .beacon_height(g as usize)
                        .map(|h| h + 2 + i as u64)
                        .unwrap_or(0);
                    let which = if i == 0 { "first" } else { "next" };
                    note.push_str(&format!(
                        "W_{g}'s {which} ask, at {}, is still due. ",
                        thousands(h)
                    ));
                }
            }
        }
        if last < 17 {
            note.push_str(&format!(
                "W_{} through W_17 are not taken to post yet.",
                last + 1
            ));
        }
        learn(&c.body, &c.root, note.trim(), on);
    }
    c.root.upcast()
}

fn words(n: usize) -> &'static str {
    match n {
        2 => "two",
        3 => "three",
        _ => "several",
    }
}

fn gossip_panel(app: &Rc<App>) -> gtk::Widget {
    let s = app.state.borrow();
    let on = s.learn();
    let (peers, header, seen) = s.produce.gossip_lines();
    drop(s);
    let c = card("Gossip", &["§9"], &[], on);
    let peers_line = line(&format!("Peers {peers}"), &["strong"]);
    let header_line = line(&format!("Header {header}"), &["mono", "small"]);
    header_line.set_wrap(true);
    let seen_line = line(&format!("Seen {seen}"), &["muted", "small"]);
    c.body.append(&peers_line);
    c.body.append(&header_line);
    c.body.append(&seen_line);
    app.tick(Page::Produce, {
        let peers_line = peers_line.clone();
        let header_line = header_line.clone();
        let seen_line = seen_line.clone();
        move |s: &State| {
            let (peers, header, seen) = s.produce.gossip_lines();
            peers_line.set_text(&format!("Peers {peers}"));
            header_line.set_text(&format!("Header {header}"));
            seen_line.set_text(&format!("Seen {seen}"));
        }
    });
    learn(
        &c.body,
        &c.root,
        "The seed is the tip the peers already have. Later headers arrive by gossip and interrupt the hike. This panel does not read the index.",
        on,
    );
    c.root.upcast()
}

fn log_drawer(app: &Rc<App>) -> gtk::Widget {
    let s = app.state.borrow();
    let written = s.produce.phase == Phase::Written;
    let title = if written {
        "Raw logs. Redacts nsec. The stderr lines above are the path record."
    } else {
        "Raw logs. Redacts nsec. Not a per-leg feed."
    };
    // Built once for the run. A refresh that rebuilt it would close it.
    let ex = log_expander(app);
    ex.set_label(Some(title));
    fill_log_body(&ex, &s.produce.log);
    app.tick(Page::Produce, {
        let ex = ex.clone();
        move |s: &State| {
            fill_log_body(&ex, &s.produce.log);
        }
    });
    ex.upcast()
}

fn log_expander(app: &Rc<App>) -> gtk::Expander {
    if let Some(existing) = app.produce_log.borrow().clone() {
        return existing;
    }
    let ex = gtk::Expander::new(None);
    ex.add_css_class("drawer");
    ex.set_resize_toplevel(false);
    let body = vbox(1);
    body.set_margin_top(6);
    ex.set_child(Some(&body));
    *app.produce_log.borrow_mut() = Some(ex.clone());
    ex
}

fn fill_log_body(ex: &gtk::Expander, log: &[String]) {
    let Some(child) = ex.child() else {
        return;
    };
    let Some(body) = child.downcast_ref::<gtk::Box>() else {
        return;
    };
    widgets::clear(body);
    let lines: Vec<&String> = log.iter().rev().take(200).collect();
    if lines.is_empty() {
        body.append(&line("No lines yet.", &["muted", "small"]));
        return;
    }
    for l in lines.into_iter().rev() {
        body.append(&line(l, &["mono", "muted"]));
    }
}

fn waypoint_chip_widget(g: u64, chip: WaypointChip) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("wp-chip");
    label.set_widget_name(&format!("wp-{g}"));
    paint_waypoint_chip(&label, chip);
    label
}

fn paint_waypoint_chip(label: &gtk::Label, chip: WaypointChip) {
    for class in [
        "wp-not-due",
        "wp-posted",
        "wp-waiting",
        "wp-anchored",
        "wp-skipped",
        "wp-post-error",
    ] {
        label.remove_css_class(class);
    }
    label.add_css_class(chip.css_class());
    label.set_text(&format!(
        "W{} {}",
        label.widget_name().trim_start_matches("wp-"),
        chip.label()
    ));
    label.set_tooltip_text(Some(chip.tooltip()));
    label.update_property(&[gtk::accessible::Property::Label(chip.tooltip())]);
}

fn waypoint_chip_legend() -> gtk::Widget {
    let items: Vec<gtk::Widget> = [
        WaypointChip::NotDueYet,
        WaypointChip::Posted,
        WaypointChip::WaitingForAnchor,
        WaypointChip::Anchored,
        WaypointChip::Skipped,
        WaypointChip::PostError,
    ]
    .into_iter()
    .map(|chip| {
        let label = gtk::Label::new(Some(chip.label()));
        label.add_css_class("wp-chip");
        label.add_css_class(chip.css_class());
        label.set_tooltip_text(Some(chip.tooltip()));
        label.upcast()
    })
    .collect();
    widgets::wrap_row(&items, 6).upcast()
}
