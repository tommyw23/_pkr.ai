use reqwest::blocking::Client;
use serde::Deserialize;
use image::DynamicImage;
use std::io::Cursor;

#[derive(Deserialize, Debug, Clone)]
pub struct PanelBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub confidence: f32,
}

pub async fn detect_panel(screenshot: &DynamicImage) -> anyhow::Result<PanelBox> {
    let screenshot_clone = screenshot.clone();
    
    let result = tokio::task::spawn_blocking(move || {
        detect_panel_blocking(&screenshot_clone)
    }).await??;
    
    Ok(result)
}

fn detect_panel_blocking(screenshot: &DynamicImage) -> anyhow::Result<PanelBox> {
    let (width, height) = (screenshot.width(), screenshot.height());
    let (resized_img, scale_factor) = if width > 1600 || height > 1200 {
        let target_width = 1280;
        let scale = target_width as f32 / width as f32;
        let target_height = (height as f32 * scale) as u32;

        let resized = screenshot.resize(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3
        );

        (resized, 1.0 / scale)
    } else {
        (screenshot.clone(), 1.0)
    };
    
    let mut png_bytes = Vec::new();
    resized_img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)?;
    
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    
    let part = reqwest::blocking::multipart::Part::bytes(png_bytes)
        .file_name("screenshot.png")
        .mime_str("image/png")?;
    
    let form = reqwest::blocking::multipart::Form::new()
        .part("file", part);
    
    let response = client
        .post("http://127.0.0.1:8000/detect")
        .multipart(form)
        .send()?;
    
    if !response.status().is_success() {
        anyhow::bail!("Detection server returned error: {}", response.status());
    }
    
    let mut panel_box: PanelBox = response.json()?;
    
    if scale_factor != 1.0 {
        panel_box.x = (panel_box.x as f32 * scale_factor) as u32;
        panel_box.y = (panel_box.y as f32 * scale_factor) as u32;
        panel_box.width = (panel_box.width as f32 * scale_factor) as u32;
        panel_box.height = (panel_box.height as f32 * scale_factor) as u32;
    }

    Ok(panel_box)
}

pub fn crop_to_panel(img: &DynamicImage, panel: &PanelBox) -> DynamicImage {
    img.crop_imm(panel.x, panel.y, panel.width, panel.height)
}