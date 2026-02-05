use std::path::{Path, PathBuf};

use eframe::egui::{Button, Modal, Response, TextEdit, Widget};
use egui::{Color32, RichText};
use egui_hooks::UseHookExt;
use egui_hooks::hook::state::Var;
use nullspace_structs::fragment::Attachment;
use nullspace_structs::username::UserName;
use poll_promise::Promise;
use pollster::FutureExt;

use crate::NullspaceApp;
use crate::promises::{PromiseSlot, flatten_rpc};
use crate::widgets::avatar::Avatar;

#[derive(Clone)]
enum AvatarChoice {
    Keep,
    Clear,
    Set(Attachment),
}

pub struct Profile<'a> {
    pub app: &'a mut NullspaceApp,
    pub open: &'a mut bool,
}

impl Widget for Profile<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let mut own_username: Var<Option<UserName>> = ui.use_state(|| None, ()).into_var();
        let username_promise = ui.use_state(PromiseSlot::new, ());
        let mut display_name_input: Var<String> = ui.use_state(String::new, ()).into_var();
        let mut initialized: Var<bool> = ui.use_state(|| false, ()).into_var();
        let mut avatar_choice: Var<AvatarChoice> =
            ui.use_state(|| AvatarChoice::Keep, ()).into_var();
        let mut avatar_upload_id: Var<Option<i64>> = ui.use_state(|| None, ()).into_var();
        let save_promise = ui.use_state(PromiseSlot::new, ());

        if *self.open {
            if own_username.is_none() && !username_promise.is_running() {
                let rpc = self.app.client.rpc();
                let promise = Promise::spawn_async(async move {
                    flatten_rpc(rpc.own_username().await)
                });
                username_promise.start(promise);
            }
            if let Some(result) = username_promise.poll() {
                match result {
                    Ok(username) => {
                        own_username.replace(username);
                    }
                    Err(err) => {
                        self.app.state.error_dialog = Some(err);
                    }
                }
            }

            Modal::new("profile_modal".into()).show(ui.ctx(), |ui| {
                ui.heading("Profile");
                ui.separator();

                let Some(username) = own_username.as_ref() else {
                    ui.label("Loading...");
                    return;
                };

                let profile_view = self
                    .app
                    .state
                    .profile_loader
                    .view(self.app.client.rpc(), username);

                if !*initialized {
                    *display_name_input = profile_view.display_name.clone().unwrap_or_default();
                    *avatar_choice = AvatarChoice::Keep;
                    *initialized = true;
                }

                ui.label(RichText::new(username.as_str()).color(Color32::GRAY));
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label("Display name");
                    ui.add(TextEdit::singleline(&mut *display_name_input).desired_width(220.0));
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Avatar");
                    let size = 32.0;
                    match &*avatar_choice {
                        AvatarChoice::Set(attachment) => {
                            ui.add(Avatar {
                                app: self.app,
                                sender: username,
                                attachment,
                                size,
                            });
                        }
                        AvatarChoice::Clear => {
                            let (rect, _) = ui.allocate_exact_size(
                                eframe::egui::vec2(size, size),
                                eframe::egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 0.0, Color32::LIGHT_GRAY);
                        }
                        AvatarChoice::Keep => {
                            if let Some(attachment) = profile_view.avatar.as_ref() {
                                ui.add(Avatar {
                                    app: self.app,
                                    sender: username,
                                    attachment,
                                    size,
                                });
                            } else {
                                let (rect, _) = ui.allocate_exact_size(
                                    eframe::egui::vec2(size, size),
                                    eframe::egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 0.0, Color32::LIGHT_GRAY);
                            }
                        }
                    }

                    if ui.button("Change…").clicked() {
                        self.app.profile_file_dialog.pick_file();
                    }
                    if ui.button("Remove").clicked() {
                        *avatar_choice = AvatarChoice::Clear;
                    }
                });

                self.app.profile_file_dialog.update(ui.ctx());
                if let Some(path) = self.app.profile_file_dialog.take_picked() {
                    start_avatar_upload(self.app, &mut avatar_upload_id, path);
                }

                if let Some(upload_id) = avatar_upload_id.as_ref() {
                    if let Some((uploaded, total)) = self.app.state.upload_progress.get(upload_id) {
                        let progress = if *total == 0 {
                            0.0
                        } else {
                            (*uploaded as f32 / *total as f32).clamp(0.0, 1.0)
                        };
                        ui.add(eframe::egui::ProgressBar::new(progress).text("Uploading..."));
                    } else if let Some(done) = self.app.state.upload_done.get(upload_id) {
                        let root = done.clone();
                        *avatar_choice = AvatarChoice::Set(root);
                        let upload_id = *upload_id;
                        *avatar_upload_id = None;
                        self.app.state.upload_done.remove(&upload_id);
                        self.app.state.upload_progress.remove(&upload_id);
                        self.app.state.upload_error.remove(&upload_id);
                    } else if let Some(error) = self.app.state.upload_error.get(upload_id) {
                        ui.label(
                            RichText::new(format!("Upload failed: {error}"))
                                .color(Color32::RED)
                                .size(11.0),
                        );
                        if ui.button("Clear error").clicked() {
                            let upload_id = *upload_id;
                            *avatar_upload_id = None;
                            self.app.state.upload_done.remove(&upload_id);
                            self.app.state.upload_progress.remove(&upload_id);
                            self.app.state.upload_error.remove(&upload_id);
                        }
                    } else {
                        ui.spinner();
                    }
                }

                ui.add_space(8.0);

                let display_name_trimmed = display_name_input.trim();
                let new_display_name = if display_name_trimmed.is_empty() {
                    None
                } else {
                    Some(display_name_trimmed.to_string())
                };

                let existing_display_name = profile_view.display_name.clone();
                let existing_avatar = profile_view.avatar.clone();

                let avatar_to_send = match &*avatar_choice {
                    AvatarChoice::Keep => existing_avatar,
                    AvatarChoice::Clear => None,
                    AvatarChoice::Set(attachment) => Some(attachment.clone()),
                };

                let display_changed = new_display_name != existing_display_name;
                let avatar_changed = match &*avatar_choice {
                    AvatarChoice::Keep => false,
                    _ => true,
                };

                let upload_busy = avatar_upload_id.is_some();
                let save_busy = save_promise.is_running();
                let can_save = (display_changed || avatar_changed) && !upload_busy && !save_busy;

                if ui.add_enabled(can_save, Button::new("Save")).clicked() {
                    let rpc = self.app.client.rpc();
                    let display_name = new_display_name.clone();
                    let avatar = avatar_to_send.clone();
                    let promise = Promise::spawn_async(async move {
                        flatten_rpc(rpc.own_profile_set(display_name, avatar).await)
                    });
                    save_promise.start(promise);
                }

                if let Some(result) = save_promise.poll() {
                    match result {
                        Ok(()) => {
                            self.app.state.error_dialog = Some("Profile updated".to_string());
                            self.app.state.profile_loader.invalidate(username);
                            *avatar_choice = AvatarChoice::Keep;
                        }
                        Err(err) => {
                            self.app.state.error_dialog = Some(err);
                        }
                    }
                }

                if ui.add(Button::new("Close")).clicked() {
                    *self.open = false;
                }
            });
        }

        ui.response()
    }
}

fn start_avatar_upload(app: &mut NullspaceApp, upload_id: &mut Var<Option<i64>>, path: PathBuf) {
    let mime = infer_mime(&path);
    if !mime.starts_with("image/") {
        app.state.error_dialog = Some("avatar must be an image".to_string());
        return;
    }
    let rpc = app.client.rpc();
    let Ok(id) = flatten_rpc(rpc.attachment_upload(path, mime).block_on()) else {
        return;
    };
    upload_id.replace(id);
}

fn infer_mime(path: &Path) -> smol_str::SmolStr {
    infer::get_from_path(path)
        .ok()
        .flatten()
        .map(|kind| smol_str::SmolStr::new(kind.mime_type()))
        .unwrap_or_else(|| smol_str::SmolStr::new("application/octet-stream"))
}
