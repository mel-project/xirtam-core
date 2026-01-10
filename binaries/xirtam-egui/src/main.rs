mod app;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "xirtam_egui=debug,xirtam_client=debug",
        ))
        .init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "xirtam-egui",
        options,
        Box::new(|cc| Ok(Box::new(app::XirtamApp::new(cc)))),
    )
}
