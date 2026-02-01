use eframe::egui::{ComboBox, Grid, Modal, Response, Widget};

use crate::{NullspaceApp, utils::prefs::{IMAGE_AUTO_DOWNLOAD_OPTIONS, label_for_auto_image_limit}};

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
                Grid::new("preferences_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
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
                        ui.end_row();

                        ui.label("Auto-download images");
                        ComboBox::from_id_salt("auto_download_images_max")
                            .selected_text(label_for_auto_image_limit(
                                self.app.state.prefs.max_auto_image_download_bytes,
                            ))
                            .show_ui(ui, |ui| {
                                for (bytes, label) in IMAGE_AUTO_DOWNLOAD_OPTIONS {
                                    ui.selectable_value(
                                        &mut self.app.state.prefs.max_auto_image_download_bytes,
                                        *bytes,
                                        *label,
                                    );
                                }
                            });
                        ui.end_row();
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
