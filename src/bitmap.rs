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
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder
        .write_header()
        .map_err(|e| anyhow::anyhow!("PNG header write failed: {}", e))?;

    let mut image_data = Vec::with_capacity(row_bytes * (bitmap.height as usize));
    for y in 0..(bitmap.height as usize) {
        image_data.extend_from_slice(&bitmap.data[y * stride..y * stride + row_bytes]);
    }
    // Convert from premultiplied (from compositing) to straight alpha for PNG.
    // Transparent pixels: ensure R=G=B=0. Opaque/semi: R = R*255/A (and clamp).
    for px in image_data.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        } else {
            let a16 = a as u16;
            px[0] = ((px[0] as u16 * 255 + a16 / 2) / a16).min(255) as u8;
            px[1] = ((px[1] as u16 * 255 + a16 / 2) / a16).min(255) as u8;
            px[2] = ((px[2] as u16 * 255 + a16 / 2) / a16).min(255) as u8;
        }
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
