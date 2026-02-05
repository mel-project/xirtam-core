use std::path::PathBuf;

use eframe::egui::{Response, Widget};
use egui_hooks::UseHookExt;
use nullspace_crypt::hash::BcsHashExt;
use nullspace_structs::fragment::Attachment;
use nullspace_structs::username::UserName;

use crate::NullspaceApp;
use crate::promises::flatten_rpc;

pub struct Avatar<'a> {
    pub app: &'a mut NullspaceApp,
    pub sender: &'a UserName,
    pub attachment: &'a Attachment,
    pub size: f32,
}

impl Widget for Avatar<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let Some(path) = avatar_cache_path(self.attachment) else {
            let (rect, response) = ui.allocate_exact_size(
                eframe::egui::vec2(self.size, self.size),
                eframe::egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, 0.0, eframe::egui::Color32::LIGHT_GRAY);
            return response;
        };

        let download_started = ui.use_state(|| false, ());
        if !path.exists() && !*download_started {
            download_started.set_next(true);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let rpc = self.app.client.rpc();
            let sender = self.sender.clone();
            let attachment = self.attachment.clone();
            let save_to = path.clone();
            tokio::spawn(async move {
                let _ = flatten_rpc(
                    rpc.attachment_download_oneshot(sender, attachment, save_to)
                        .await,
                );
            });
        }

        if path.exists() {
            #[cfg(windows)]
            let path_str = path.to_string_lossy().replace('\\', "/");
            #[cfg(not(windows))]
            let path_str = path.to_string_lossy().to_string();
            let uri = format!("file://{path_str}");
            ui.add(
                eframe::egui::Image::from_uri(uri)
                    .fit_to_exact_size(eframe::egui::vec2(self.size, self.size))
                    .show_loading_spinner(false),
            )
        } else {
            let (rect, response) = ui.allocate_exact_size(
                eframe::egui::vec2(self.size, self.size),
                eframe::egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, 0.0, eframe::egui::Color32::LIGHT_GRAY);
            response
        }
    }
}

fn avatar_cache_path(attachment: &Attachment) -> Option<PathBuf> {
    let base = dirs::cache_dir()?;
    let filename = attachment.bcs_hash().to_string();
    Some(base.join("nullspace").join("avatars").join(filename))
}
