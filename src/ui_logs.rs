use crate::logs::{self, Query, SpanId};
use egui::{TextStyle, Ui};
use egui_extras::{Size, StripBuilder};

use super::App;

#[derive(Debug, Default, Clone)]
pub struct LogsUi {
    cur_span: Option<SpanId>,
}

impl App {
    pub fn ui_logs(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        // Print the logs
        StripBuilder::new(ui)
            .size(Size::exact(180.0))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|ui| self.ui_logs_list(ui, ctx));
                strip.cell(|ui| self.ui_logs_text(ui, ctx));
            });
    }

    fn ui_logs_list(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        ui.push_id(1, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label("choose a span: ");
                ui.add_space(10.0);

                let ui_state = &mut self.logs_ui;
                let logs = self.logs.inner.lock().unwrap();
                for (span_id, entry) in &logs.spans {
                    let mut header = String::new();
                    logs::print_span_header(&mut header, 0, entry);
                    if ui.link(header.trim()).clicked() {
                        ui_state.cur_span = Some(*span_id);
                    }
                }
            });
        });
    }

    fn ui_logs_text(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        let ui_state = &mut self.logs_ui;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let query = if let Some(span) = ui_state.cur_span {
                Query::Span(span)
            } else {
                Query::All
            };
            let text = self.logs.string_query(query);
            ui.add(
                egui::TextEdit::multiline(&mut &**text)
                    .font(TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
        });
    }
}
