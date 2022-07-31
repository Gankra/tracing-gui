use crate::logs::{Query, SpanId};
use egui::{TextStyle, Ui};

use super::App;

#[derive(Debug, Default, Clone)]
pub struct LinearLogsUi {
    cur_span: Option<SpanId>,
}

impl App {
    pub fn ui_logs_linear(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        // Print the logs
        self.ui_logs_linear_text(ui, ctx)
    }

    fn ui_logs_linear_text(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        ui.label("TODO");
        let ui_state = &mut self.linear_logs_ui;
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
