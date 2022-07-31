use egui::Ui;

use crate::ProcessorStatus;

use super::App;

#[derive(Debug, Default, Clone)]
pub struct SettingsUi {}

impl App {
    pub fn ui_settings(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.add_space(20.0);
        ui.heading("choose log.json");
        ui.add_space(10.0);

        // Show a listing of currently known minidumps to inspect
        let mut do_set_path = None;
        for (i, path) in self.settings.available_paths.iter().enumerate() {
            if ui
                .button(&*path.file_name().unwrap().to_string_lossy())
                .clicked()
            {
                do_set_path = Some(i);
            }
        }
        if let Some(i) = do_set_path {
            self.set_path(i);
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            // ui.label(message);

            let cancellable = matches!(self.cur_status, ProcessorStatus::Reading);
            ui.add_enabled_ui(cancellable, |ui| {
                if ui.button("❌ cancel").clicked() {
                    self.cancel_processing();
                }
            });
            /*
            let reprocessable = matches!(&self.minidump, Some(Ok(_)));
            ui.add_enabled_ui(reprocessable, |ui| {
                if ui.button("💫 reprocess").clicked() {
                    self.process_dump(self.minidump.as_ref().unwrap().as_ref().unwrap().clone());
                }
            });
             */
        });

        ui.add_space(10.0);

        if ui.button("Open log file...").clicked() {
            // FIXME(WASM): this has to be made async in wasm
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                self.settings.available_paths.push(path);
                self.set_path(self.settings.available_paths.len() - 1);
            }
        }

        ui.add_space(20.0);
        preview_files_being_dropped(ctx);

        // Collect dropped files:
        let mut pushed_path = false;
        for file in &ctx.input().raw.dropped_files {
            if let Some(path) = &file.path {
                pushed_path = true;
                self.settings.available_paths.push(path.clone());
            }
        }
        if pushed_path {
            self.set_path(self.settings.available_paths.len() - 1);
        }
    }
}

fn preview_files_being_dropped(ctx: &egui::Context) {
    use egui::*;
    use std::fmt::Write as _;

    if !ctx.input().raw.hovered_files.is_empty() {
        let mut text = "Dropping files:\n".to_owned();
        for file in &ctx.input().raw.hovered_files {
            if let Some(path) = &file.path {
                write!(text, "\n{}", path.display()).ok();
            } else if !file.mime.is_empty() {
                write!(text, "\n{}", file.mime).ok();
            } else {
                text += "\n???";
            }
        }

        let painter =
            ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

        let screen_rect = ctx.input().screen_rect();
        painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(192));
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Heading.resolve(&ctx.style()),
            Color32::WHITE,
        );
    }
}
