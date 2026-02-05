use std::path::PathBuf;

use eframe::egui::{Response, Widget};
use egui_hooks::UseHookExt;
use nullspace_crypt::hash::BcsHashExt;
use nullspace_structs::fragment::Attachment;
use nullspace_structs::username::UserName;

use crate::promises::flatten_rpc;
use crate::rpc::get_rpc;
use crate::widgets::smooth::SmoothImage;

pub struct Avatar<'a> {
    pub sender: &'a UserName,
    pub attachment: &'a Attachment,
    pub size: f32,
}

impl Widget for Avatar<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let radius_u8 = (self.size / 2.0).round().clamp(0.0, u8::MAX as f32) as u8;
        let circle_corner_radius = eframe::egui::CornerRadius::same(radius_u8);
        let Some(path) = avatar_cache_path(self.attachment) else {
            let (rect, response) = ui.allocate_exact_size(
                eframe::egui::vec2(self.size, self.size),
                eframe::egui::Sense::hover(),
            );
            paint_avatar_placeholder(ui.painter(), rect);
            return response;
        };

        let download_started = ui.use_state(|| false, ());
        if !path.exists() && !*download_started {
            download_started.set_next(true);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let sender = self.sender.clone();
            let attachment = self.attachment.clone();
            let save_to = path.clone();
            tokio::spawn(async move {
                let _ = flatten_rpc(
                    get_rpc().attachment_download_oneshot(sender, attachment, save_to)
                        .await,
                );
            });
        }

        if path.exists() {
            let max_size = eframe::egui::vec2(self.size, self.size);
            ui.add(SmoothImage {
                filename: path.as_path(),
                max_size,
                corner_radius: circle_corner_radius,
            })
        } else {
            let (rect, response) = ui.allocate_exact_size(
                eframe::egui::vec2(self.size, self.size),
                eframe::egui::Sense::hover(),
            );
            paint_avatar_placeholder(ui.painter(), rect);
            response
        }
    }
}

fn paint_avatar_placeholder(painter: &eframe::egui::Painter, rect: eframe::egui::Rect) {
    let radius = rect.width().min(rect.height()) / 2.0;
    painter.circle_filled(rect.center(), radius, eframe::egui::Color32::LIGHT_GRAY);
}

fn avatar_cache_path(attachment: &Attachment) -> Option<PathBuf> {
    let base = dirs::cache_dir()?;
    let filename = attachment.bcs_hash().to_string();
    Some(base.join("nullspace").join("avatars").join(filename))
}
