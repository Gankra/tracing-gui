use crate::logs::{self, Query, SpanId};
use egui::{TextStyle, Ui};

use super::App;

#[derive(Debug, Default, Clone)]
pub struct TreeLogsUi {
    cur_span: Option<SpanId>,
}

impl App {
    pub fn ui_logs_tree(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        // Print the logs
        egui::SidePanel::left("my_left_panel")
            .show_inside(ui, |ui| self.ui_logs_tree_list(ui, ctx));
        egui::CentralPanel::default().show_inside(ui, |ui| self.ui_logs_tree_text(ui, ctx));
    }

    fn ui_logs_tree_list(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        ui.push_id(1, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label("choose a span: ");
                ui.add_space(10.0);

                let ui_state = &mut self.tree_logs_ui;
                let logs = self.logs.inner.lock().unwrap();
                for (span_id, entry) in &logs.spans {
                    let mut header = String::new();
                    logs::print_span_header(&mut header, 0, entry, false);
                    if ui.link(header).clicked() {
                        ui_state.cur_span = Some(*span_id);
                    }
                }
            });
        });
    }

    fn ui_logs_tree_text(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        let ui_state = &mut self.tree_logs_ui;
        egui::ScrollArea::both()
            .auto_shrink([true; 2])
            .show(ui, |ui| {
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
