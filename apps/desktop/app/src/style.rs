//! "Night trail" (DESIGN-v2 section 4). One stylesheet, two palettes.
//! The semantic colours are the same roles in both themes, darkened for
//! text on the light paper.

use pcoc_hike_core::config::Theme;

struct Palette {
    ground: &'static str,
    side: &'static str,
    card: &'static str,
    raised: &'static str,
    line: &'static str,
    text: &'static str,
    muted: &'static str,
    faint: &'static str,
    teal: &'static str,
    teal_fg: &'static str,
    violet: &'static str,
    amber: &'static str,
    blue: &'static str,
    magenta: &'static str,
    code: &'static str,
}

const DARK: Palette = Palette {
    ground: "#0c1210",
    side: "#0e1613",
    card: "#16211d",
    raised: "#1d2a25",
    line: "#26352f",
    text: "#e8f3ec",
    muted: "#9db1a7",
    faint: "#4b5d55",
    teal: "#6ee0c5",
    teal_fg: "#062b22",
    violet: "#cbb8f6",
    amber: "#f0c14d",
    blue: "#8ec8f2",
    magenta: "#e56aa8",
    code: "#0f1815",
};

const LIGHT: Palette = Palette {
    ground: "#f3efe6",
    side: "#ebe5d8",
    card: "#fffcf7",
    raised: "#f6f1e7",
    line: "#ddd5c5",
    text: "#1b2420",
    muted: "#5d6863",
    faint: "#b9b2a4",
    teal: "#0c7a64",
    teal_fg: "#ffffff",
    violet: "#6946b8",
    amber: "#8a5a08",
    blue: "#1a6294",
    magenta: "#b4236a",
    code: "#f1ece1",
};

pub fn css(theme: Theme) -> String {
    let p = match theme {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    };
    let mut out = format!(
        "@define-color pc_ground {};\n@define-color pc_side {};\n@define-color pc_card {};\n@define-color pc_raised {};\n\
         @define-color pc_line {};\n@define-color pc_text {};\n@define-color pc_muted {};\n@define-color pc_faint {};\n\
         @define-color pc_teal {};\n@define-color pc_teal_fg {};\n@define-color pc_violet {};\n@define-color pc_amber {};\n\
         @define-color pc_blue {};\n@define-color pc_magenta {};\n@define-color pc_code {};\n",
        p.ground, p.side, p.card, p.raised, p.line, p.text, p.muted, p.faint, p.teal, p.teal_fg, p.violet, p.amber,
        p.blue, p.magenta, p.code
    );
    out.push_str(BASE);
    out
}

const BASE: &str = r#"
@define-color window_bg_color @pc_ground;
@define-color window_fg_color @pc_text;
@define-color view_bg_color @pc_card;
@define-color view_fg_color @pc_text;
@define-color card_bg_color @pc_card;
@define-color card_fg_color @pc_text;
@define-color headerbar_bg_color @pc_ground;
@define-color headerbar_fg_color @pc_text;
@define-color headerbar_border_color @pc_line;
@define-color headerbar_shade_color @pc_line;
@define-color sidebar_bg_color @pc_side;
@define-color sidebar_fg_color @pc_text;
@define-color popover_bg_color @pc_card;
@define-color popover_fg_color @pc_text;
@define-color dialog_bg_color @pc_card;
@define-color dialog_fg_color @pc_text;
@define-color accent_bg_color @pc_teal;
@define-color accent_fg_color @pc_teal_fg;
@define-color accent_color @pc_teal;

window, .ground { background-color: @pc_ground; color: @pc_text; }
headerbar { background-color: @pc_ground; box-shadow: inset 0 -1px @pc_line; min-height: 52px; }
.page-title { font-weight: 700; font-size: 11.5pt; }
.page-sub { color: @pc_muted; font-size: 11.5pt; }

.sidebar { background-color: @pc_side; border-right: 1px solid @pc_line; padding: 14px 10px 12px 10px; }
.sidebar.rail { padding-left: 6px; padding-right: 6px; }
.brand-name { font-weight: 800; font-size: 11.5pt; }
.brand-sub { color: @pc_muted; font-size: 8.5pt; }
.logo-bar { border-radius: 2px; min-width: 4px; }
.logo-teal { background-color: @pc_teal; }
.logo-amber { background-color: @pc_amber; }
.logo-blue { background-color: @pc_blue; }
.nav { background: none; }
.nav row { border-radius: 8px; padding: 7px 8px; margin: 1px 0; border: 1px solid transparent; }
.nav row:hover { background-color: alpha(@pc_teal, 0.06); }
.nav row:selected { background-color: alpha(@pc_teal, 0.13); border-color: alpha(@pc_teal, 0.45); color: @pc_text; }
.nav row:selected image { color: @pc_teal; }
.nav-label { font-weight: 600; font-size: 10pt; }
.badge { background-color: alpha(@pc_teal, 0.16); color: @pc_teal; border-radius: 999px; padding: 1px 8px; font-size: 8.5pt; font-weight: 700; }
.side-head { color: @pc_muted; font-size: 8pt; font-weight: 800; letter-spacing: 1px; margin: 14px 6px 6px 6px; }
.objects { background: none; }
.objects row { border-radius: 6px; padding: 5px 6px; }
.objects row:selected { background-color: alpha(@pc_teal, 0.09); color: @pc_text; }
.obj-name { font-weight: 600; font-size: 9.5pt; }
.obj-line { color: @pc_muted; font-size: 8.5pt; }

.card-pc { background-color: @pc_card; border: 1px solid @pc_line; border-radius: 12px; padding: 12px 14px; }
.card-title { color: @pc_muted; font-size: 8.5pt; font-weight: 800; letter-spacing: 1px; }
.chip { border: 1px solid @pc_line; border-radius: 999px; padding: 0 7px; font-family: "Liberation Mono", monospace; font-size: 8pt; color: @pc_muted; }
.token { background-color: @pc_raised; border: 1px solid @pc_line; border-radius: 5px; padding: 1px 6px; font-family: "Liberation Mono", monospace; font-size: 8.5pt; }
.token.mismatch { color: @pc_magenta; border-color: alpha(@pc_magenta, 0.6); }
.hero { font-size: 22pt; font-weight: 700; font-feature-settings: "tnum"; }
.big { font-size: 13pt; font-weight: 700; font-feature-settings: "tnum"; }
.num { font-weight: 700; font-feature-settings: "tnum"; }
.lead { font-size: 10.5pt; }
.strong { font-weight: 700; }
.muted { color: @pc_muted; }
.learn { color: @pc_muted; font-size: 8.8pt; }
.small { font-size: 8.8pt; }
.mono { font-family: "Liberation Mono", monospace; font-size: 9pt; }
.teal { color: @pc_teal; }
.amber { color: @pc_amber; }
.violet { color: @pc_violet; }
.blue { color: @pc_blue; }
.magenta { color: @pc_magenta; }
.origin { font-size: 20pt; font-weight: 700; }
.code-block { background-color: @pc_code; border: 1px solid @pc_line; border-radius: 6px; padding: 8px 10px; }
.sep { background-color: @pc_line; min-height: 1px; }

button.primary { background-color: @pc_teal; color: @pc_teal_fg; font-weight: 700; border-radius: 7px; padding: 5px 12px; }
button.primary:disabled { background-color: alpha(@pc_teal, 0.25); color: alpha(@pc_teal_fg, 0.7); }
button.outline { background: none; border: 1px solid @pc_line; border-radius: 7px; padding: 5px 12px; font-weight: 600; }
button.outline:hover { background-color: alpha(@pc_text, 0.05); }
button.danger { background: none; border: 1px solid alpha(@pc_magenta, 0.55); border-radius: 7px; padding: 5px 12px; font-weight: 600; }
.seg button { border-radius: 0; padding: 4px 12px; background: none; border: 1px solid @pc_line; font-weight: 600; }
.seg button:first-child { border-radius: 7px 0 0 7px; }
.seg button:last-child { border-radius: 0 7px 7px 0; border-left: none; }
.seg button:checked { background-color: @pc_text; color: @pc_ground; }
entry, text, textview, passwordentry { font-size: 10pt; }
.link-box { background-color: @pc_code; border: 1px solid @pc_line; border-radius: 6px; padding: 6px 8px; }
.link-box text { background-color: @pc_code; }

.pill { background-color: @pc_card; border: 1px solid @pc_line; border-radius: 999px; padding: 3px 10px 3px 6px; }
.pill-words { font-weight: 700; font-size: 8.5pt; }
.pill-npub { font-family: "Liberation Mono", monospace; font-size: 8.5pt; color: @pc_muted; }
.ident-px { min-width: 3px; min-height: 3px; }
.ident-big .ident-px { min-width: 5px; min-height: 5px; }
.ident-on { background-color: @pc_text; }
.ident-on.teal-px { background-color: @pc_teal; }
.ident-on.amber-px { background-color: @pc_amber; }

.statusbar { border-top: 1px solid @pc_line; padding: 7px 14px; font-size: 9pt; }
.wrap-row { }
.wrap-row > * { margin: 2px 6px 2px 0; }
.dot { min-width: 8px; min-height: 8px; border-radius: 999px; background-color: @pc_faint; }
.dot.amber { background-color: @pc_amber; }
.dot.blue { background-color: @pc_blue; }
.dot.teal { background-color: @pc_teal; }

.cell { min-width: 12px; min-height: 12px; border-radius: 2px; border: 1px solid transparent; }
.cell-served { background-color: @pc_amber; }
.cell-miss { border-color: @pc_amber; background-image: repeating-linear-gradient(135deg, @pc_amber, @pc_amber 1.5px, transparent 1.5px, transparent 4px); }
.cell-due { border: 1px dotted @pc_amber; }
.cell-notdue { border-color: alpha(@pc_faint, 0.9); }
.cell-dot { min-width: 2px; min-height: 2px; border-radius: 999px; background-color: @pc_faint; }
.cell-stored { border-color: alpha(@pc_faint, 0.9); }
.cell-above { border: 1px dashed @pc_faint; }
.w-name { color: @pc_violet; font-weight: 700; font-size: 8.5pt; font-family: "Liberation Mono", monospace; }
.skip-word { font-family: "Liberation Mono", monospace; font-size: 8.5pt; }
.wp-chip { border-radius: 999px; padding: 2px 8px; font-size: 8pt; font-weight: 700; }
.wp-not-due { background-color: alpha(@pc_faint, 0.35); color: @pc_muted; }
.wp-posted { background-color: alpha(@pc_amber, 0.28); color: @pc_amber; }
.wp-waiting { background-color: alpha(@pc_blue, 0.28); color: @pc_blue; }
.wp-anchored { background-color: alpha(#f7931a, 0.30); color: #f7931a; }
.wp-skipped { background-color: alpha(@pc_violet, 0.28); color: @pc_violet; }
.wp-post-error { background-color: alpha(@pc_magenta, 0.28); color: @pc_magenta; }

.bar { background-color: @pc_teal; border-radius: 2px 2px 0 0; min-width: 4px; }
.bar-open { background-color: transparent; background-image: repeating-linear-gradient(135deg, @pc_teal, @pc_teal 3px, alpha(@pc_teal, 0.25) 3px, alpha(@pc_teal, 0.25) 6px); }
.bar-count { font-weight: 700; font-size: 9pt; font-feature-settings: "tnum"; }
.bar-stage { color: @pc_muted; font-size: 7.5pt; }
.int-stored { min-width: 6px; min-height: 6px; border-radius: 1px; background-color: @pc_amber; }
.int-not { min-width: 5px; min-height: 5px; border-radius: 999px; border: 1px dashed @pc_amber; }
.legend-swatch { min-width: 8px; min-height: 8px; border-radius: 2px; }
.sw-teal { background-color: @pc_teal; }
.sw-open { background-image: repeating-linear-gradient(135deg, @pc_teal, @pc_teal 2px, alpha(@pc_teal, 0.25) 2px, alpha(@pc_teal, 0.25) 4px); }

.count-cell { background-color: @pc_raised; border-radius: 6px; padding: 4px 2px; }
.count-cell.shaded { background-color: alpha(@pc_teal, 0.14); }
.count-cell .num { font-size: 10.5pt; }

.rail .nav row { padding: 9px 0; }
expander title { color: @pc_muted; }
.drawer { background-color: @pc_card; border: 1px solid @pc_line; border-radius: 8px; padding: 6px 10px; }
"#;
