use eframe::egui::{ComboBox, Modal, Response, Widget};

use crate::NullspaceApp;

pub struct Preferences<'a> {
    pub app: &'a mut NullspaceApp,
    pub open: &'a mut bool,
}

impl Widget for Preferences<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        if *self.open {
            let modal = Modal::new("preferences_modal".into()).show(ui.ctx(), |ui| {
                ui.heading("Preferences");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Zoom");
                    ComboBox::from_id_salt("zoom_percent")
                        .selected_text(format!("{}%", self.app.state.prefs.zoom_percent))
                        .show_ui(ui, |ui| {
                            for percent in (75u16..=200).step_by(25) {
                                ui.selectable_value(
                                    &mut self.app.state.prefs.zoom_percent,
                                    percent,
                                    format!("{percent}%"),
                                );
                            }
                        });
                });
                if ui.button("Close").clicked() {
                    *self.open = false;
                }
            });
            if modal.should_close() {
                *self.open = false;
            }
        }
        ui.response()
    }
}
