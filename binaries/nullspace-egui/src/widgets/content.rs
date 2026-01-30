use crate::utils::color::username_color;
use crate::widgets::convo::default_download_dir;
use eframe::egui::{Response, RichText, Widget};
use egui::{Color32, ImageSource, TextFormat};
use egui::{TextStyle, text::LayoutJob};
use egui_hooks::UseHookExt;
use nullspace_client::internal::MessageContent;
use nullspace_crypt::hash::Hash;
use pollster::FutureExt;

use crate::NullspaceApp;
use crate::promises::flatten_rpc;
use crate::utils::markdown::layout_md_raw;
use crate::utils::speed::speed_fmt;
use crate::utils::units::{format_filesize, unit_for_bytes};

pub struct Content<'a> {
    pub app: &'a mut NullspaceApp,
    pub message: &'a nullspace_client::internal::ConvoMessage,
}

impl Widget for Content<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let font_id = ui
            .style()
            .text_styles
            .get(&TextStyle::Body)
            .cloned()
            .unwrap();

        let mut base_text_format = TextFormat {
            color: Color32::BLACK,
            font_id,
            ..Default::default()
        };
        if self.message.send_error.is_some() {
            base_text_format.strikethrough = egui::Stroke::new(1.0, Color32::BLACK);
        }
        let sender_color = username_color(&self.message.sender);

        ui.horizontal_top(|ui| {
            ui.colored_label(sender_color, format!("{}: ", self.message.sender));
            ui.vertical(|ui| {
                match &self.message.body {
                    MessageContent::GroupInvite { invite_id } => {
                        ui.horizontal_top(|ui| {
                            ui.colored_label(Color32::GRAY, "Invitation to group");
                            if ui.link("Accept").clicked() {
                                let rpc = self.app.client.rpc();
                                let invite_id = *invite_id;
                                tokio::spawn(async move {
                                    let _ = flatten_rpc(rpc.group_accept_invite(invite_id).await);
                                });
                            }
                        });
                    }
                    MessageContent::Attachment { id, size, mime } => {
                        ui.add(AttachmentContent {
                            app: self.app,
                            message: self.message,
                            id: *id,
                            size: *size,
                            mime,
                        });
                    }
                    MessageContent::PlainText(text) => {
                        let mut job = LayoutJob::default();
                        job.append(text, 0.0, base_text_format.clone());
                        ui.label(job);
                    }
                    MessageContent::Markdown(text) => {
                        let mut job = LayoutJob::default();
                        layout_md_raw(&mut job, base_text_format.clone(), text);
                        ui.label(job);
                    }
                };

                if let Some(err) = &self.message.send_error {
                    ui.label(
                        RichText::new(format!("Send failed: {err}"))
                            .color(Color32::RED)
                            .size(11.0),
                    );
                }
            })
        });
        ui.response()
    }
}

pub struct AttachmentContent<'a> {
    pub app: &'a mut NullspaceApp,
    pub message: &'a nullspace_client::internal::ConvoMessage,
    pub id: Hash,
    pub size: u64,
    pub mime: &'a str,
}

impl Widget for AttachmentContent<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let status = ui.use_memo(
            || flatten_rpc(self.app.client.rpc().attachment_status(self.id).block_on()),
            self.app.state.attach_updates,
        );
        let dl_path = status.as_ref().ok().and_then(|s| s.saved_to.as_ref());
        let dl_progress = self
            .app
            .state
            .download_progress
            .get(&self.id)
            .map(|(downloaded, total)| (*downloaded, *total));
        let dl_error = self.app.state.download_error.get(&self.id);
        let image_downloading = ui.use_state(|| false, ());

        defmac::defmac!(start_dl => {
            let save_dir = default_download_dir();
            let rpc = self.app.client.rpc();
            let _ = flatten_rpc(rpc.attachment_download(self.id, save_dir).block_on());
        });
        let (unit_scale, unit_suffix) = unit_for_bytes(self.size);
        let size_text = format_filesize(self.size, unit_scale);
        let attachment_label = format!("[{} {} {}]", self.mime, size_text, unit_suffix);
        if self.mime.starts_with("image/") && self.size < 20_000_000 {
            ui.label(attachment_label);
            if !*image_downloading {
                image_downloading.set_next(true);
                start_dl!();
            }
            if let Some(path) = dl_path {
                if let Ok(image) = self.app.images.get_or_load(ui.ctx(), path) {
                    let max_box = egui::vec2(600.0, 400.0);
                    ui.add(egui::Image::from_texture(&image).fit_to_exact_size(max_box));
                }
            }
        } else {
            ui.horizontal_top(|ui| {
                ui.colored_label(Color32::DARK_BLUE, attachment_label);
                if let Ok(status) = status
                    && let Some(path) = status.saved_to
                {
                    if ui.small_button("Open").clicked() {
                        let _ = open::that_detached(path);
                    }
                } else if ui.small_button("Download").clicked() {
                    start_dl!();
                }
            });
        }
        if let Some((downloaded, total)) = dl_progress {
            let speed_key = format!("download-{}", self.id);
            let (left, speed, right) = speed_fmt(&speed_key, downloaded, total);
            let speed_text = format!("Downloading: {left} @ {speed}, {right} remaining");
            ui.label(
                RichText::new(speed_text.to_string())
                    .color(Color32::GRAY)
                    .size(11.0),
            );
            ui.ctx().request_repaint();
        } else if let Some(error) = dl_error {
            ui.label(
                RichText::new(format!("Download failed: {error}"))
                    .color(Color32::RED)
                    .size(11.0),
            );
        }
        ui.response()
    }
}
