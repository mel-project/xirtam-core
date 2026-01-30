use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use egui::TextureHandle;

pub struct ImageCache {
    map: HashMap<PathBuf, TextureHandle>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get_or_load(
        &mut self,
        ctx: &egui::Context,
        path: impl Into<PathBuf>,
    ) -> anyhow::Result<TextureHandle> {
        let path = path.into();

        if let Some(tex) = self.map.get(&path) {
            return Ok(tex.clone());
        }

        let tex = load_texture_from_path(ctx, &path)?;
        self.map.insert(path, tex.clone());
        Ok(tex)
    }
}

fn load_texture_from_path(ctx: &egui::Context, path: &Path) -> anyhow::Result<TextureHandle> {
    let img = image::open(path)?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Ok(ctx.load_texture(path.to_string_lossy(), color_image, Default::default()))
}
