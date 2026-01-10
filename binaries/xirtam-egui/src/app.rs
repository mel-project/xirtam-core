use std::collections::BTreeSet;
use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bytes::Bytes;
use eframe::egui;
use egui_flex::{Flex, FlexAlignContent, item};
use tokio::runtime::{Handle as TokioHandle, Runtime};
use tokio::time;
use url::Url;

use xirtam_client::internal::{
    DmDirection, DmMessage, Event, NewDeviceBundle, RegisterFinish, RegisterStartInfo,
};
use xirtam_client::{Client, Config, InternalClient};
use xirtam_crypt::signing::SigningPublic;
use xirtam_structs::gateway::GatewayName;
use xirtam_structs::handle::Handle;
use xirtam_structs::timestamp::Timestamp;

#[derive(Debug)]
enum UiMsg {
    RegisterStart(Result<Option<RegisterStartInfo>, String>),
    RegisterFinish(Result<(), String>),
    Bundle(Result<NewDeviceBundle, String>),
    DmHistory {
        peer: String,
        result: Result<Vec<DmMessage>, String>,
    },
    DmSend(Result<i64, String>),
}

pub struct XirtamApp {
    runtime_handle: TokioHandle,
    client: Option<Client>,
    rpc: Option<Arc<InternalClient>>,
    events_rx: Receiver<Event>,
    events_tx: Sender<Event>,
    ui_rx: Receiver<UiMsg>,
    ui_tx: Sender<UiMsg>,
    ui_ctx: Option<egui::Context>,
    logged_in: bool,
    status: String,
    db_path: String,
    dir_endpoint: String,
    dir_anchor_pk: String,
    register_handle: String,
    register_gateway: String,
    register_info: Option<RegisterStartInfo>,
    bundle_input: String,
    bundle_output: String,
    bundle_can_sign: bool,
    bundle_expiry_secs: String,
    peers: BTreeSet<String>,
    selected_peer: Option<String>,
    manual_peer_input: String,
    message_input: String,
    mime_input: String,
    messages: Vec<DmMessage>,
}

impl XirtamApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (events_tx, events_rx) = async_channel::unbounded();
        let (ui_tx, ui_rx) = async_channel::unbounded();
        cc.egui_ctx.set_zoom_factor(1.0);
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let runtime = Runtime::new().expect("tokio runtime");
        let handle = runtime.handle().clone();
        std::thread::spawn(move || {
            runtime.block_on(async { pending::<()>().await });
        });
        Self {
            runtime_handle: handle,
            client: None,
            rpc: None,
            events_rx,
            events_tx,
            ui_rx,
            ui_tx,
            ui_ctx: None,
            logged_in: false,
            status: "not started".to_string(),
            db_path: "xirtam-client.db".to_string(),
            dir_endpoint: "http://127.0.0.1:4000".to_string(),
            dir_anchor_pk: "OnF3Jh7tZ4o3g3bmUdgjTDa4qlMHN-2Q4RrJQD6K124".to_string(),
            register_handle: "@alice01".to_string(),
            register_gateway: "~demo01".to_string(),
            register_info: None,
            bundle_input: String::new(),
            bundle_output: String::new(),
            bundle_can_sign: false,
            bundle_expiry_secs: "31536000".to_string(),
            peers: BTreeSet::new(),
            selected_peer: None,
            manual_peer_input: "@bob01".to_string(),
            message_input: String::new(),
            mime_input: "text/plain".to_string(),
            messages: Vec::new(),
        }
    }

    fn start_client(&mut self, ctx: &egui::Context) {
        let dir_endpoint = match Url::parse(&self.dir_endpoint) {
            Ok(value) => value,
            Err(err) => {
                self.status = format!("invalid dir endpoint: {err}");
                return;
            }
        };
        let dir_anchor_pk = match self.dir_anchor_pk.parse::<SigningPublic>() {
            Ok(value) => value,
            Err(err) => {
                self.status = format!("invalid dir anchor pk: {err}");
                return;
            }
        };
        let config = Config {
            db_path: self.db_path.clone().into(),
            dir_endpoint,
            dir_anchor_pk,
        };
        let _guard = self.runtime_handle.enter();
        let client = Client::new(config);
        let rpc = Arc::new(client.rpc());
        self.client = Some(client);
        self.rpc = Some(Arc::clone(&rpc));
        self.status = "client started".to_string();
        self.spawn_event_listener(rpc, ctx.clone());
    }

    fn spawn_event_listener(&self, rpc: Arc<InternalClient>, ctx: egui::Context) {
        let sender = self.events_tx.clone();
        let handle = self.runtime_handle.clone();
        std::thread::spawn(move || {
            handle.block_on(async move {
                loop {
                    match rpc.next_event().await {
                        Ok(event) => {
                            if sender.send(event).await.is_err() {
                                break;
                            }
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "event loop error");
                        }
                    }
                    ctx.request_repaint();
                }
            });
        });
    }

    fn spawn_ui_task<F>(&self, fut: F)
    where
        F: std::future::Future<Output = UiMsg> + Send + 'static,
    {
        let sender = self.ui_tx.clone();
        let handle = self.runtime_handle.clone();
        let ctx = self.ui_ctx.clone();
        handle.spawn(async move {
            tracing::debug!("ui task started");
            let msg = fut.await;
            tracing::debug!("ui task completed");
            let _ = sender.send(msg).await;
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.events_rx.try_recv() {
            tracing::debug!(event = ?event, "event received");
            match event {
                Event::State { logged_in } => {
                    self.logged_in = logged_in;
                    self.status = if logged_in {
                        "logged in".to_string()
                    } else {
                        "logged out".to_string()
                    };
                }
                Event::DmUpdated { peer } => {
                    self.peers.insert(peer.as_str().to_string());
                    if self.selected_peer.as_deref() == Some(peer.as_str()) {
                        self.request_history(peer.as_str());
                    }
                }
            }
        }
    }

    fn drain_ui_msgs(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            tracing::debug!(msg = ?msg, "ui msg received");
            match msg {
                UiMsg::RegisterStart(result) => match result {
                    Ok(Some(info)) => {
                        self.register_gateway = info.gateway_name.as_str().to_string();
                        self.register_info = Some(info);
                        self.status = "handle exists".to_string();
                    }
                    Ok(None) => {
                        self.register_info = None;
                        self.status = "handle available".to_string();
                    }
                    Err(err) => {
                        self.status = format!("register_start error: {err}");
                    }
                },
                UiMsg::RegisterFinish(result) => match result {
                    Ok(()) => self.status = "registration complete".to_string(),
                    Err(err) => self.status = format!("register_finish error: {err}"),
                },
                UiMsg::Bundle(result) => match result {
                    Ok(bundle) => {
                        self.bundle_output = URL_SAFE_NO_PAD.encode(&bundle.0);
                        self.status = "bundle created".to_string();
                    }
                    Err(err) => self.status = format!("bundle error: {err}"),
                },
                UiMsg::DmHistory { peer, result } => {
                    if self.selected_peer.as_deref() == Some(peer.as_str()) {
                        match result {
                            Ok(messages) => self.messages = messages,
                            Err(err) => self.status = format!("dm_history error: {err}"),
                        }
                    }
                }
                UiMsg::DmSend(result) => match result {
                    Ok(_id) => {
                        self.message_input.clear();
                        if let Some(peer) = self.selected_peer.clone() {
                            self.request_history(&peer);
                        }
                    }
                    Err(err) => self.status = format!("dm_send error: {err}"),
                },
            }
        }
    }

    fn request_history(&mut self, peer: &str) {
        let Some(rpc) = self.rpc.clone() else {
            return;
        };
        let peer_handle = match Handle::parse(peer) {
            Ok(value) => value,
            Err(_) => {
                self.status = "invalid peer handle".to_string();
                return;
            }
        };
        let peer_key = peer.to_string();
        self.spawn_ui_task(async move {
            let result = rpc.dm_history(peer_handle, None, None, 200).await;
            UiMsg::DmHistory {
                peer: peer_key,
                result: flatten_rpc(result),
            }
        });
    }

    fn ui_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("status: {}", self.status));
            if self.logged_in {
                ui.label("state: logged in");
            } else {
                ui.label("state: logged out");
            }
        });
    }

    fn ui_config(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.group(|ui| {
            ui.label("Client config");
            ui.horizontal(|ui| {
                ui.label("DB path");
                ui.text_edit_singleline(&mut self.db_path);
            });
            ui.horizontal(|ui| {
                ui.label("Dir endpoint");
                ui.text_edit_singleline(&mut self.dir_endpoint);
            });
            ui.horizontal(|ui| {
                ui.label("Dir anchor pk");
                ui.text_edit_singleline(&mut self.dir_anchor_pk);
            });
            if ui.button("Start client").clicked() {
                self.start_client(ctx);
            }
        });
    }

    fn ui_register(&mut self, ui: &mut egui::Ui) {
        let Some(rpc) = self.rpc.clone() else {
            ui.label("Start client to register.");
            return;
        };
        ui.group(|ui| {
            ui.label("Registration");
            ui.horizontal(|ui| {
                ui.label("Handle");
                ui.text_edit_singleline(&mut self.register_handle);
                if ui.button("Check").clicked() {
                    let rpc = Arc::clone(&rpc);
                    tracing::debug!(handle = %self.register_handle, "register_start clicked");
                    self.status = "checking handle...".to_string();
                    let handle = match Handle::parse(&self.register_handle) {
                        Ok(value) => value,
                        Err(_) => {
                            self.status = "invalid handle".to_string();
                            return;
                        }
                    };
                    self.spawn_ui_task(async move {
                        tracing::debug!("register_start rpc begin");
                        let result = time::timeout(
                            Duration::from_secs(10),
                            rpc.register_start(handle),
                        )
                        .await;
                        tracing::debug!("register_start rpc done");
                        let result = match result {
                            Ok(inner) => flatten_rpc(inner),
                            Err(_) => Err("register_start timeout".to_string()),
                        };
                        UiMsg::RegisterStart(result)
                    });
                }
            });

            ui.horizontal(|ui| {
                ui.label("Gateway");
                ui.text_edit_singleline(&mut self.register_gateway);
                if ui.button("Register new handle").clicked() {
                    let rpc = Arc::clone(&rpc);
                    let handle = match Handle::parse(&self.register_handle) {
                        Ok(value) => value,
                        Err(_) => {
                            self.status = "invalid handle".to_string();
                            return;
                        }
                    };
                    let gateway = match GatewayName::parse(&self.register_gateway) {
                        Ok(value) => value,
                        Err(_) => {
                            self.status = "invalid gateway".to_string();
                            return;
                        }
                    };
                    self.spawn_ui_task(async move {
                        let result = rpc
                            .register_finish(RegisterFinish::BootstrapNewHandle {
                                handle,
                                gateway_name: gateway,
                            })
                            .await;
                        UiMsg::RegisterFinish(flatten_rpc(result))
                    });
                }
            });

            ui.separator();
            ui.label("Add device (paste bundle)");
            ui.text_edit_multiline(&mut self.bundle_input);
            if ui.button("Register device").clicked() {
                let rpc = Arc::clone(&rpc);
                let decoded = match URL_SAFE_NO_PAD.decode(self.bundle_input.trim()) {
                    Ok(value) => value,
                    Err(err) => {
                        self.status = format!("bundle decode error: {err}");
                        return;
                    }
                };
                let bundle = NewDeviceBundle(Bytes::from(decoded));
                self.spawn_ui_task(async move {
                    let result = rpc.register_finish(RegisterFinish::AddDevice { bundle }).await;
                    UiMsg::RegisterFinish(flatten_rpc(result))
                });
            }
        });
    }

    fn ui_bundle(&mut self, ui: &mut egui::Ui) {
        if !self.logged_in {
            ui.label("Login to create a device bundle.");
            return;
        }
        let Some(rpc) = self.rpc.clone() else {
            return;
        };
        ui.group(|ui| {
            ui.label("New device bundle");
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.bundle_can_sign, "Can sign");
                ui.label("Expiry (secs from now)");
                ui.text_edit_singleline(&mut self.bundle_expiry_secs);
                if ui.button("Generate").clicked() {
                    let rpc = Arc::clone(&rpc);
                    let ttl: u64 = match self.bundle_expiry_secs.trim().parse() {
                        Ok(value) => value,
                        Err(_) => {
                            self.status = "invalid expiry".to_string();
                            return;
                        }
                    };
                    let now = Timestamp::now().0;
                    let expiry = Timestamp(now.saturating_add(ttl));
                    let can_sign = self.bundle_can_sign;
                    self.spawn_ui_task(async move {
                        let result = rpc.new_device_bundle(can_sign, expiry).await;
                        UiMsg::Bundle(flatten_rpc(result))
                    });
                }
            });
            ui.label("Bundle (share as QR)");
            ui.text_edit_multiline(&mut self.bundle_output);
        });
    }

    fn ui_dm(&mut self, ui: &mut egui::Ui) {
        let Some(rpc) = self.rpc.clone() else {
            ui.label("Start client to use DMs.");
            return;
        };
        ui.group(|ui| {
            ui.label("Direct messages");
            Flex::horizontal().show(ui, |flex| {
                flex.add_flex(
                    item().basis(180.0),
                    Flex::vertical().align_content(FlexAlignContent::Stretch),
                    |flex| {
                        flex.add(item(), egui::Label::new("Peers"));
                        flex.add_ui(item(), |ui| {
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.manual_peer_input);
                                if ui.button("Open").clicked() {
                                    let peer = match Handle::parse(&self.manual_peer_input) {
                                        Ok(value) => value,
                                        Err(_) => {
                                            self.status = "invalid peer handle".to_string();
                                            return;
                                        }
                                    };
                                    let key = peer.as_str().to_string();
                                    self.peers.insert(key.clone());
                                    self.selected_peer = Some(key.clone());
                                    self.request_history(&key);
                                }
                            });
                        });
                        flex.add_ui(item().grow(1.0), |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("peers_scroll")
                                .show(ui, |ui| {
                                    for peer in self.peers.clone() {
                                        let selected =
                                            self.selected_peer.as_deref() == Some(peer.as_str());
                                    let response = ui.selectable_label(selected, peer.clone());
                                    if response.clicked() {
                                        self.selected_peer = Some(peer.clone());
                                        self.request_history(&peer);
                                    }
                                }
                            });
                        });
                    },
                );

                flex.add_flex(
                    item().grow(1.0),
                    Flex::vertical().align_content(FlexAlignContent::Stretch),
                    |flex| {
                        flex.add(item(), egui::Label::new("Messages"));
                        flex.add_ui(item().grow(1.0), |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("messages_scroll")
                                .show(ui, |ui| {
                                    for msg in &self.messages {
                                        ui.horizontal(|ui| {
                                            let prefix = match msg.direction {
                                                DmDirection::Incoming => "<-",
                                                DmDirection::Outgoing => "->",
                                            };
                                            ui.label(prefix);
                                            ui.label(msg.sender.as_str());
                                            ui.label(msg.mime.as_str());
                                            ui.label(body_preview(msg));
                                        });
                                    }
                                });
                        });
                        flex.add_ui(item(), |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Mime");
                                ui.text_edit_singleline(&mut self.mime_input);
                                ui.label("Message");
                                ui.text_edit_singleline(&mut self.message_input);
                                if ui.button("Send").clicked() {
                                    let rpc = Arc::clone(&rpc);
                                    let Some(peer) = self.selected_peer.clone() else {
                                        self.status = "select a peer".to_string();
                                        return;
                                    };
                                    let peer = match Handle::parse(peer) {
                                        Ok(value) => value,
                                        Err(_) => {
                                            self.status = "invalid peer".to_string();
                                            return;
                                        }
                                    };
                                    let mime = smol_str::SmolStr::new(self.mime_input.clone());
                                    let body = Bytes::from(self.message_input.clone().into_bytes());
                                    self.spawn_ui_task(async move {
                                        let result = rpc.dm_send(peer, mime, body).await;
                                        UiMsg::DmSend(flatten_rpc(result))
                                    });
                                }
                            });
                        });
                    },
                );
            });
        });
    }
}

fn body_preview(msg: &DmMessage) -> String {
    match std::str::from_utf8(&msg.body) {
        Ok(text) => truncate_text(text, 120),
        Err(_) => format!("<{} bytes>", msg.body.len()),
    }
}

fn truncate_text(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    let mut out = value[..max_len].to_string();
    out.push_str("...");
    out
}

impl eframe::App for XirtamApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui_ctx = Some(ctx.clone());
        self.drain_events();
        self.drain_ui_msgs();
        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui_header(ui);
            self.ui_config(ui, ctx);
            self.ui_register(ui);
            self.ui_bundle(ui);
            self.ui_dm(ui);
        });
    }
}

fn flatten_rpc<T, E: std::fmt::Display>(
    result: Result<Result<T, xirtam_client::internal::InternalRpcError>, E>,
) -> Result<T, String> {
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(err.to_string()),
        Err(err) => Err(err.to_string()),
    }
}
