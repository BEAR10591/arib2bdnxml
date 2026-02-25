//! RGBA bitmap to PNG output (using the png crate).

use std::fs::File;
use std::io::BufWriter;

/// RGBA bitmap (stride bytes per row).
#[derive(Debug, Clone)]
pub struct BitmapData {
    pub data: Vec<u8>,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
}

/// Save bitmap as PNG.
pub fn save_bitmap_as_png(bitmap: &BitmapData, path: &str) -> anyhow::Result<()> {
    if bitmap.data.is_empty() || bitmap.width <= 0 || bitmap.height <= 0 {
        anyhow::bail!("Invalid bitmap data.");
    }
    let w = bitmap.width as u32;
    let h = bitmap.height as u32;
    let stride = bitmap.stride as usize;
    let row_bytes = (bitmap.width as usize) * 4;

    let file = File::create(path)
        .map_err(|e| anyhow::anyhow!("Failed to open file: {}: {}", path, e))?;
    let mut out = BufWriter::new(file);

    let mut encoder = png::Encoder::new(&mut out, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| anyhow::anyhow!("PNG header write failed: {}", e))?;

    let mut image_data = Vec::with_capacity(row_bytes * (bitmap.height as usize));
    for y in 0..(bitmap.height as usize) {
        image_data.extend_from_slice(&bitmap.data[y * stride..y * stride + row_bytes]);
    }
    writer
        .write_image_data(&image_data)
        .map_err(|e| anyhow::anyhow!("PNG write failed: {}", e))?;
    writer.finish().map_err(|e| anyhow::anyhow!("PNG finish: {}", e))?;
    Ok(())
}

/// Format: base_name + zero-padded 5-digit index + ".png"
pub fn generate_png_filename(index: usize, base_name: &str) -> String {
    format!("{}{:05}.png", base_name, index)
}
