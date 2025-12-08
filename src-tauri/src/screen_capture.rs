// src-tauri/src/screen_capture.rs
// Handles DPI scale factor detection and coordinate conversion for high-DPI displays

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PhysicalCoordinates {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogicalCoordinates {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Get the DPI scale factor for the primary monitor
/// Returns 1.0 on error (fallback to no scaling)
#[tauri::command]
pub fn get_dpi_scale_factor() -> Result<f64, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, HORZRES, DESKTOPHORZRES};
        use windows::Win32::Foundation::HWND;

        unsafe {
            let hdc = GetDC(HWND(0));
            if hdc.is_invalid() {
                return Ok(1.0);
            }

            // Get logical and physical screen width
            let logical_width = GetDeviceCaps(hdc, HORZRES);
            let physical_width = GetDeviceCaps(hdc, DESKTOPHORZRES);

            if logical_width > 0 {
                let scale = physical_width as f64 / logical_width as f64;
                println!("🔍 DPI Scale Factor detected: {:.2}x (logical: {}, physical: {})",
                    scale, logical_width, physical_width);
                Ok(scale)
            } else {
                Ok(1.0)
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS typically uses 2.0 for Retina displays
        // We can get this from the NSScreen backingScaleFactor
        // For now, return 1.0 as fallback - can be enhanced later
        Ok(1.0)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(1.0)
    }
}

/// Convert logical window coordinates to physical screen coordinates
/// Logical coords are what Tauri window API returns (e.g., 2880×1856 on high-DPI)
/// Physical coords are what screenshot capture uses (e.g., 5760×3712 on 2x scaling)
pub fn logical_to_physical(
    logical: &LogicalCoordinates,
    scale_factor: f64,
) -> PhysicalCoordinates {
    PhysicalCoordinates {
        x: (logical.x.max(0) as f64 * scale_factor).round() as u32,
        y: (logical.y.max(0) as f64 * scale_factor).round() as u32,
        width: (logical.width as f64 * scale_factor).round() as u32,
        height: (logical.height as f64 * scale_factor).round() as u32,
    }
}

/// Convert physical screen coordinates back to logical coordinates
pub fn physical_to_logical(
    physical: &PhysicalCoordinates,
    scale_factor: f64,
) -> LogicalCoordinates {
    LogicalCoordinates {
        x: (physical.x as f64 / scale_factor).round() as i32,
        y: (physical.y as f64 / scale_factor).round() as i32,
        width: (physical.width as f64 / scale_factor).round() as u32,
        height: (physical.height as f64 / scale_factor).round() as u32,
    }
}

/// Capture a specific detection region using physical coordinates
/// This ensures the crop coordinates match exactly with the screenshot pixels
pub async fn capture_detection_region(
    logical_bounds: &LogicalCoordinates,
) -> Result<image::DynamicImage, String> {
    use screenshots::Screen;

    // Get DPI scale factor
    let scale_factor = get_dpi_scale_factor().unwrap_or(1.0);

    println!("📐 Logical bounds: x={}, y={}, w={}, h={}",
        logical_bounds.x, logical_bounds.y, logical_bounds.width, logical_bounds.height);

    // Convert to physical coordinates
    let physical = logical_to_physical(logical_bounds, scale_factor);

    println!("📐 Physical bounds ({}x scale): x={}, y={}, w={}, h={}",
        scale_factor, physical.x, physical.y, physical.width, physical.height);

    // Capture full screen
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens.first().ok_or("No screens found")?;
    let full_image = screen.capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    println!("📸 Full screenshot size: {}x{}", full_image.width(), full_image.height());

    // Convert to image buffer
    let img_buffer = image::RgbaImage::from_raw(
        full_image.width(),
        full_image.height(),
        full_image.rgba().to_vec(),
    ).ok_or("Failed to create image buffer")?;

    let dynamic_img = image::DynamicImage::ImageRgba8(img_buffer);

    // Ensure physical coordinates are within image bounds
    let crop_x = physical.x.min(dynamic_img.width().saturating_sub(1));
    let crop_y = physical.y.min(dynamic_img.height().saturating_sub(1));
    let crop_width = physical.width.min(dynamic_img.width() - crop_x);
    let crop_height = physical.height.min(dynamic_img.height() - crop_y);

    println!("✂️  Cropping to: x={}, y={}, w={}, h={}", crop_x, crop_y, crop_width, crop_height);

    // Crop to the detection region
    let cropped = dynamic_img.crop_imm(crop_x, crop_y, crop_width, crop_height);

    println!("✅ Cropped image size: {}x{}", cropped.width(), cropped.height());

    Ok(cropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_to_physical_2x_scaling() {
        let logical = LogicalCoordinates {
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };

        let physical = logical_to_physical(&logical, 2.0);

        assert_eq!(physical.x, 200);
        assert_eq!(physical.y, 400);
        assert_eq!(physical.width, 1600);
        assert_eq!(physical.height, 1200);
    }

    #[test]
    fn test_physical_to_logical_2x_scaling() {
        let physical = PhysicalCoordinates {
            x: 200,
            y: 400,
            width: 1600,
            height: 1200,
        };

        let logical = physical_to_logical(&physical, 2.0);

        assert_eq!(logical.x, 100);
        assert_eq!(logical.y, 200);
        assert_eq!(logical.width, 800);
        assert_eq!(logical.height, 600);
    }

    #[test]
    fn test_negative_coordinates_handled() {
        let logical = LogicalCoordinates {
            x: -50,
            y: -100,
            width: 800,
            height: 600,
        };

        let physical = logical_to_physical(&logical, 2.0);

        // Negative coordinates should be clamped to 0
        assert_eq!(physical.x, 0);
        assert_eq!(physical.y, 0);
    }
}
