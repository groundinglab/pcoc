//! Small building blocks shared by the pages.

use gtk::prelude::*;
use gtk::{glib, pango};
use pcoc_hike_core::record::explain;

pub fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

pub fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

pub fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Plain text, left aligned, wrapping.
pub fn text(s: &str, classes: &[&str]) -> gtk::Label {
    let l = gtk::Label::new(None);
    l.set_markup(&spec_markup(s));
    l.set_xalign(0.0);
    l.set_wrap(true);
    l.set_wrap_mode(pango::WrapMode::WordChar);
    for c in classes {
        l.add_css_class(c);
    }
    l
}

/// One line that does not wrap.
pub fn line(s: &str, classes: &[&str]) -> gtk::Label {
    let l = text(s, classes);
    l.set_wrap(false);
    l
}

/// Selectable monospace text, for hashes and paths. Never shortened here.
pub fn mono(s: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(s));
    l.set_xalign(0.0);
    l.set_selectable(true);
    l.set_wrap(true);
    l.set_wrap_mode(pango::WrapMode::Char);
    let attrs = pango::AttrList::new();
    attrs.insert(pango::AttrInt::new_insert_hyphens(false));
    l.set_attributes(Some(&attrs));
    l.add_css_class("mono");
    l
}

/// `I_5` and `W_17` as subscripts, the way the SPEC prints them. Other
/// underscores (token names such as `whole_hike_span`) are left alone.
pub fn spec_markup(s: &str) -> String {
    let escaped = glib::markup_escape_text(s).to_string();
    let chars: Vec<char> = escaped.chars().collect();
    let mut out = String::with_capacity(escaped.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let prev_alnum = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
        if matches!(c, 'I' | 'W' | 'P' | 'T' | 'd') && !prev_alnum && chars.get(i + 1) == Some(&'_')
        {
            let mut j = i + 2;
            let braced = chars.get(j) == Some(&'{');
            if braced {
                j += 1;
            }
            let start = j;
            while j < chars.len()
                && (chars[j].is_ascii_digit()
                    || matches!(chars[j], 'g' | 'a' | 'b' | 'k')
                    || (braced && matches!(chars[j], '−' | '-' | '+')))
            {
                j += 1;
            }
            let ok_end = if braced {
                chars.get(j) == Some(&'}')
            } else {
                true
            };
            let next_alnum = chars
                .get(j)
                .is_some_and(|n| n.is_alphanumeric() || *n == '_');
            if j > start && ok_end && (braced || !next_alnum) {
                let sub: String = chars[start..j].iter().collect();
                out.push(c);
                out.push_str("<sub>");
                out.push_str(&sub);
                out.push_str("</sub>");
                i = if braced { j + 1 } else { j };
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

pub fn chip(s: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(s));
    l.add_css_class("chip");
    l.set_valign(gtk::Align::Center);
    l
}

/// A §14 token, with its meaning as a tooltip in both modes.
pub fn token(s: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(s));
    l.add_css_class("token");
    if s == "mismatch" {
        l.add_css_class("mismatch");
    }
    l.set_valign(gtk::Align::Center);
    let meaning = explain(s);
    if !meaning.is_empty() {
        l.set_tooltip_text(Some(meaning));
    }
    l
}

pub struct Card {
    pub root: gtk::Box,
    pub head: gtk::Box,
    pub body: gtk::Box,
}

/// A card with an uppercase title and SPEC chips. `note_chips` are the
/// descriptive chips that Learn mode off removes; section chips stay.
pub fn card(title: &str, section_chips: &[&str], note_chips: &[&str], learn_on: bool) -> Card {
    let root = vbox(8);
    root.add_css_class("card-pc");
    let head = hbox(8);
    if !title.is_empty() {
        let t = line(&title.to_uppercase(), &["card-title"]);
        head.append(&t);
    }
    for c in section_chips.iter().filter(|c| !c.is_empty()) {
        head.append(&chip(c));
    }
    if learn_on {
        for c in note_chips {
            head.append(&chip(c));
        }
    }
    if head.first_child().is_some() {
        root.append(&head);
    }
    let body = vbox(6);
    root.append(&body);
    Card { root, head, body }
}

/// Learn mode on: the explanation is on the page. Off: it is the tooltip of
/// `owner`. Tooltips exist in both modes.
pub fn learn(container: &gtk::Box, owner: &impl IsA<gtk::Widget>, s: &str, on: bool) {
    let owner = owner.as_ref();
    let tip = match owner.tooltip_text() {
        Some(t) if !t.is_empty() => format!("{t}\n{s}"),
        _ => s.to_owned(),
    };
    owner.set_tooltip_text(Some(&tip.replace('_', "")));
    if on {
        container.append(&text(s, &["learn"]));
    }
}

pub fn button(label: &str, class: &str) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class(class);
    b
}

pub fn spacer() -> gtk::Box {
    let b = hbox(0);
    b.set_hexpand(true);
    b
}

pub fn separator() -> gtk::Box {
    let b = hbox(0);
    b.add_css_class("sep");
    b
}

/// A key/value grid with a muted key column.
pub fn kv(rows: &[(&str, gtk::Widget)], key_width: i32) -> gtk::Grid {
    let g = gtk::Grid::new();
    g.set_row_spacing(6);
    g.set_column_spacing(16);
    for (i, (k, v)) in rows.iter().enumerate() {
        let key = line(k, &["muted", "small"]);
        key.set_width_request(key_width);
        key.set_valign(gtk::Align::Start);
        g.attach(&key, 0, i as i32, 1, 1);
        v.set_hexpand(true);
        g.attach(v, 1, i as i32, 1, 1);
    }
    g
}

/// A 5×5 mirrored identicon from the public key bytes. Decoration only;
/// the npub is printed next to it.
pub fn identicon(public_key: Option<&[u8; 32]>, colour: &str, big: bool) -> gtk::Grid {
    let g = gtk::Grid::new();
    g.set_row_spacing(1);
    g.set_column_spacing(1);
    g.set_valign(gtk::Align::Center);
    if big {
        g.add_css_class("ident-big");
    }
    let bits = public_key
        .map(|k| {
            k[..4]
                .iter()
                .fold(0u32, |acc, b| (acc << 8) | u32::from(*b))
        })
        .unwrap_or(0);
    for y in 0..5 {
        for x in 0..5 {
            let half = if x < 3 { x } else { 4 - x };
            let on = public_key.is_some() && (bits >> (y * 3 + half)) & 1 == 1;
            let px = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            px.add_css_class("ident-px");
            if on {
                px.add_css_class("ident-on");
                px.add_css_class(colour);
            }
            g.attach(&px, x, y, 1, 1);
        }
    }
    g
}

pub fn status_dot(colour: Option<&str>) -> gtk::Box {
    let d = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    d.add_css_class("dot");
    if let Some(c) = colour {
        d.add_css_class(c);
    }
    d.set_valign(gtk::Align::Center);
    d
}

/// A row of equal-width columns. Children wrap onto the next line, so a
/// narrow window does not grow to fit them.
pub fn columns(children: &[gtk::Widget], spacing: i32) -> gtk::Widget {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(true);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(children.len().max(1) as u32);
    flow.set_column_spacing(spacing as u32);
    flow.set_row_spacing(spacing as u32);
    flow.set_hexpand(true);
    flow.add_css_class("wrap-row");
    for c in children {
        c.set_hexpand(true);
        flow.insert(c, -1);
    }
    flow.upcast()
}

/// A horizontal row that wraps. Used where a plain box would request more
/// width than the window has.
pub fn wrap_row(children: &[gtk::Widget], spacing: i32) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(false);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(children.len().max(1) as u32);
    flow.set_column_spacing(spacing as u32);
    flow.set_row_spacing(4);
    flow.set_hexpand(true);
    flow.add_css_class("wrap-row");
    for c in children {
        flow.insert(c, -1);
    }
    flow
}
