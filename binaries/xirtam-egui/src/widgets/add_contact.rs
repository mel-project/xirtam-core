use eframe::egui::{Button, Modal, Response, Spinner, TextEdit, Widget};
use egui_hooks::UseHookExt;
use egui_hooks::hook::state::Var;
use poll_promise::Promise;
use xirtam_structs::handle::Handle;

use crate::XirtamApp;
use crate::promises::{PromiseSlot, flatten_rpc};

pub struct AddContact<'a> {
    pub app: &'a mut XirtamApp,
    pub open: &'a mut bool,
}

impl Widget for AddContact<'_> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let mut handle_str: Var<String> = ui.use_state(String::new, ()).into_var();
        let mut message_str: Var<String> = ui.use_state(String::new, ()).into_var();
        let add_contact = ui.use_state(PromiseSlot::new, ());

        if *self.open {
            Modal::new("add_contact_modal".into()).show(ui.ctx(), |ui| {
                ui.heading("Add contact");
                let busy = add_contact.is_running();
                ui.horizontal(|ui| {
                    ui.label("Handle");
                    ui.add_enabled(
                        !busy,
                        TextEdit::singleline(&mut *handle_str).desired_width(240.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Message");
                    ui.add_enabled(
                        !busy,
                        TextEdit::multiline(&mut *message_str).desired_width(240.0),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!busy, Button::new("Cancel")).clicked() {
                        *self.open = false;
                    }
                    if ui.add_enabled(!busy, Button::new("Add")).clicked() {
                        let handle = match Handle::parse(handle_str.trim()) {
                            Ok(handle) => handle,
                            Err(err) => {
                                self.app.state.error_dialog =
                                    Some(format!("invalid handle: {err}"));
                                return;
                            }
                        };
                        let init_msg = message_str.clone();
                        let rpc = self.app.client.rpc();
                        let promise = Promise::spawn_async(async move {
                            flatten_rpc(rpc.add_contact(handle, init_msg).await)
                        });
                        add_contact.start(promise);
                    }
                });
                if add_contact.is_running() {
                    ui.add(Spinner::new());
                }
                if let Some(result) = add_contact.poll() {
                    match result {
                        Ok(()) => {
                            *self.open = false;
                            handle_str.clear();
                            message_str.clear();
                        }
                        Err(err) => {
                            self.app.state.error_dialog = Some(err);
                        }
                    }
                }
            });
        }
        ui.response()
    }
}
