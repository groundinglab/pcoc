//! One window, one screen. Buttons use the labels from the library.
#![windows_subsystem = "windows"]

use eframe::egui;
use pcoc_windows::session::StartOpts;
use pcoc_windows::{
    Session, LABEL_FOLDER, LABEL_GENERATE, LABEL_HISTORY, LABEL_SAVE, LABEL_START, LABEL_STOP,
};
use pcoc_windows_core::history::CLEARTEXT_NOTE;
use pcoc_windows_core::signin;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PCOC")
            .with_inner_size([760.0, 920.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PCOC",
        options,
        Box::new(|_creation| Ok(Box::new(PcocApp::new()))),
    )
}

struct PcocApp {
    session: Session,
}

impl PcocApp {
    fn new() -> Self {
        Self {
            session: Session::new(None),
        }
    }
}

impl eframe::App for PcocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.session.poll();
        if self.session.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PCOC");
            ui.label(CLEARTEXT_NOTE);
            ui.label(self.session.history_display());
            ui.separator();
            ui.label("Key");
            let edit = egui::TextEdit::singleline(&mut self.session.nsec_input)
                .hint_text("nsec")
                .desired_width(f32::INFINITY);
            if ui.add(edit).changed() {
                self.session.refresh_key();
            }
            ui.horizontal(|ui| {
                if ui.button(LABEL_GENERATE).clicked() {
                    self.session.generate();
                }
                if ui.button(LABEL_SAVE).clicked() {
                    self.session.save_key();
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut self.session.save_path)
                    .hint_text("Path for Save key. Nothing is written until you press Save key.")
                    .desired_width(f32::INFINITY),
            );
            if !self.session.npub.is_empty() {
                ui.label(format!("npub: {}", self.session.npub));
            }
            if !self.session.key_message.is_empty() {
                ui.label(&self.session.key_message);
            }
            ui.separator();
            ui.horizontal(|ui| {
                let start = egui::Button::new(LABEL_START);
                if ui.add_enabled(!self.session.is_running(), start).clicked() {
                    self.session.start(StartOpts::live());
                }
                if ui.button(LABEL_STOP).clicked() {
                    self.session.stop();
                }
            });
            ui.label(&self.session.status);
            ui.horizontal(|ui| {
                if ui.button(LABEL_HISTORY).clicked() {
                    self.session.toggle_history();
                }
                if ui.button(LABEL_FOLDER).clicked() {
                    self.session.open_history_folder();
                }
            });
            ui.label(format!(
                "History folder: {}",
                self.session.history_display()
            ));
            if self.session.show_history {
                if self.session.history_lines.is_empty() {
                    ui.label("No saved hikes in that folder.");
                }
                for line in &self.session.history_lines {
                    ui.monospace(line);
                }
            }
            ui.separator();
            ui.label(format!("Sign-in origin: {}", signin::ORIGIN));
            ui.add(
                egui::TextEdit::singleline(&mut self.session.sign_in_link)
                    .hint_text("Sign-in link from the website")
                    .desired_width(f32::INFINITY),
            );
            if !self.session.report.is_empty() {
                ui.separator();
                ui.label(&self.session.report);
            }
        });
    }
}
