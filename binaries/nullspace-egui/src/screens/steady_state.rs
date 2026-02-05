use eframe::egui::{Response, ViewportCommand, Widget};
use egui::{Align, Button, Layout};
use egui_hooks::UseHookExt;
use egui_hooks::hook::state::Var;
use nullspace_client::internal::{ConvoId, ConvoSummary};

use std::sync::Arc;

use crate::NullspaceApp;
use crate::promises::flatten_rpc;
use crate::widgets::add_contact::AddContact;
use crate::widgets::add_device::AddDevice;
use crate::widgets::add_group::AddGroup;
use crate::widgets::convo::Convo;
use crate::widgets::profile::Profile;
use crate::widgets::preferences::Preferences;

pub struct SteadyState<'a>(pub &'a mut NullspaceApp);

impl Widget for SteadyState<'_> {
    fn ui(mut self, ui: &mut eframe::egui::Ui) -> Response {
        let rpc = Arc::new(self.0.client.rpc());
        let mut selected_chat: Var<Option<ConvoId>> = ui.use_state(|| None, ()).into_var();
        let mut show_add_contact: Var<bool> = ui.use_state(|| false, ()).into_var();
        let mut show_add_group: Var<bool> = ui.use_state(|| false, ()).into_var();
        let mut show_add_device: Var<bool> = ui.use_state(|| false, ()).into_var();
        let mut show_preferences: Var<bool> = ui.use_state(|| false, ()).into_var();
        let mut show_profile: Var<bool> = ui.use_state(|| false, ()).into_var();
        let convos = ui.use_memo(
            || {
                let result = pollster::block_on(rpc.convo_list());
                flatten_rpc(result)
            },
            self.0.state.msg_updates,
        );

        let frame = eframe::egui::Frame::default().inner_margin(eframe::egui::Margin::same(8));
        eframe::egui::TopBottomPanel::top("steady_menu").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Preferences").clicked() {
                        *show_preferences = true;
                        ui.close();
                    }
                    if ui.button("Profile…").clicked() {
                        *show_profile = true;
                        ui.close();
                    }
                    if ui.button("Add device").clicked() {
                        *show_add_device = true;
                        ui.close();
                    }
                    if ui.button("Exit").clicked() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        ui.close();
                    }
                });
            });
            ui.add_space(4.0);
        });
        eframe::egui::SidePanel::left("steady_left")
            .resizable(false)
            .exact_width(200.0)
            .frame(frame)
            .show_inside(ui, |ui| {
                self.render_left(
                    ui,
                    &convos,
                    &mut selected_chat,
                    &mut show_add_contact,
                    &mut show_add_group,
                )
            });
        eframe::egui::CentralPanel::default()
            .frame(frame)
            .show_inside(ui, |ui| {
                self.render_right(ui, &selected_chat);
            });
        ui.add(AddContact {
            app: self.0,
            open: &mut show_add_contact,
        });
        ui.add(AddGroup {
            app: self.0,
            open: &mut show_add_group,
        });
        ui.add(AddDevice {
            app: self.0,
            open: &mut show_add_device,
        });
        ui.add(Preferences {
            app: self.0,
            open: &mut show_preferences,
        });
        ui.add(Profile {
            app: self.0,
            open: &mut show_profile,
        });
        ui.response()
    }
}

impl<'a> SteadyState<'a> {
    fn render_left(
        &mut self,
        ui: &mut eframe::egui::Ui,
        convos: &Result<Vec<ConvoSummary>, String>,
        selected_chat: &mut Option<ConvoId>,
        show_add_contact: &mut bool,
        show_add_group: &mut bool,
    ) {
        ui.horizontal(|ui| {
            if ui.add(Button::new("Add contact")).clicked() {
                *show_add_contact = true;
            }
            if ui.add(Button::new("New group")).clicked() {
                *show_add_group = true;
            }
        });
        ui.separator();
        match convos {
            Ok(lst) => {
                let mut direct_base_names: std::collections::HashMap<
                    nullspace_structs::username::UserName,
                    String,
                > = std::collections::HashMap::new();
                let mut direct_counts: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();

                for convo in lst {
                    if let ConvoId::Direct { peer } = &convo.convo_id {
                        let label = self
                            .0
                            .state
                            .profile_loader
                            .label_for(self.0.client.rpc(), peer)
                            .display;
                        direct_counts
                            .entry(label.clone())
                            .and_modify(|count| *count += 1)
                            .or_insert(1);
                        direct_base_names.insert(peer.clone(), label);
                    }
                }
                ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                    for convo in lst {
                        let selection = convo.convo_id.clone();
                        let label = match &convo.convo_id {
                            ConvoId::Direct { peer } => {
                                let base = direct_base_names
                                    .get(peer)
                                    .cloned()
                                    .unwrap_or_else(|| peer.to_string());
                                if direct_counts.get(&base).copied().unwrap_or(0) > 1 {
                                    format!("{base} ({peer})")
                                } else {
                                    base
                                }
                            }
                            ConvoId::Group { group_id } => {
                                format!("Group {}", short_group_id(group_id))
                            }
                        };
                        if ui
                            .selectable_label(*selected_chat == Some(selection.clone()), label)
                            .clicked()
                        {
                            selected_chat.replace(selection);
                        }
                    }
                });
            }
            Err(err) => {
                self.0.state.error_dialog.replace(err.to_string());
            }
        }
    }

    fn render_right(&mut self, ui: &mut eframe::egui::Ui, selected_chat: &Option<ConvoId>) {
        if let Some(selection) = selected_chat {
            ui.add(Convo(self.0, selection.clone()));
        }
    }
}

fn short_group_id(group: &nullspace_structs::group::GroupId) -> String {
    let bytes = group.as_bytes();
    let mut out = String::with_capacity(8);
    for byte in bytes.iter().take(4) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
