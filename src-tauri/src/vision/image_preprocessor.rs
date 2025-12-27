// src-tauri/src/vision/image_preprocessor.rs
// Image preprocessing for vision API optimization
// Resize, enhance contrast/brightness before sending to OpenAI/Claude

use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

/// Target dimensions for vision API input
/// 1280×720 provides good detail while minimizing token usage
const TARGET_WIDTH: u32 = 1280;
const TARGET_HEIGHT: u32 = 720;

/// Contrast boost amount (0.0 = no change, positive = more contrast)
const CONTRAST_BOOST: f32 = 10.0;

/// Brightness adjustment (0.0 = no change, positive = brighter)
const BRIGHTNESS_BOOST: i32 = 5;

/// Configuration for image preprocessing
#[derive(Debug, Clone)]
pub struct PreprocessConfig {
    pub target_width: u32,
    pub target_height: u32,
    pub contrast_boost: f32,
    pub brightness_boost: i32,
    pub enable_resize: bool,
    pub enable_contrast: bool,
    pub enable_brightness: bool,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        Self {
            target_width: TARGET_WIDTH,
            target_height: TARGET_HEIGHT,
            contrast_boost: CONTRAST_BOOST,
            brightness_boost: BRIGHTNESS_BOOST,
            enable_resize: true,
            enable_contrast: false,  // Disabled for speed - minimal accuracy benefit
            enable_brightness: false, // Disabled for speed - minimal accuracy benefit
        }
    }
}

impl PreprocessConfig {
    /// Create a site-specific preprocessing configuration
    /// Higher resolution for sites with smaller card graphics (e.g., Replay Poker)
    pub fn for_site(site_name: Option<&str>) -> Self {
        let (width, height) = match site_name {
            // Replay Poker has smaller card graphics, needs higher resolution
            Some("replay") => (1920, 1080),
            // All other sites work well with standard 1280x720
            _ => (TARGET_WIDTH, TARGET_HEIGHT),
        };

        Self {
            target_width: width,
            target_height: height,
            contrast_boost: CONTRAST_BOOST,
            brightness_boost: BRIGHTNESS_BOOST,
            enable_resize: true,
            enable_contrast: false,
            enable_brightness: false,
        }
    }
}

/// Preprocess image for vision API consumption
/// 1. Resize to optimal dimensions (1280×720)
/// 2. Apply contrast boost for better card/text recognition
/// 3. Apply brightness adjustment for consistent lighting
pub fn preprocess_for_vision_api(
    image: &DynamicImage,
    config: &PreprocessConfig,
) -> DynamicImage {
    let mut processed = image.clone();

    // STEP 1: Resize to target dimensions
    if config.enable_resize {
        let (original_width, original_height) = (processed.width(), processed.height());
        let original_aspect = original_width as f32 / original_height as f32;
        let target_aspect = config.target_width as f32 / config.target_height as f32;

        // Calculate dimensions that fit within target while maintaining aspect ratio
        let (resize_width, resize_height) = if original_aspect > target_aspect {
            // Image is wider than target - fit to width
            let width = config.target_width.min(original_width);
            let height = (width as f32 / original_aspect) as u32;
            (width, height)
        } else {
            // Image is taller than target - fit to height
            let height = config.target_height.min(original_height);
            let width = (height as f32 * original_aspect) as u32;
            (width, height)
        };

        if resize_width != original_width || resize_height != original_height {
            // Use Nearest neighbor for maximum speed (~0.1-0.2s vs 2-3s for Triangle)
            // Vision AI models don't need high-quality interpolation - they work fine with blocky resizes
            processed = processed.resize(
                resize_width,
                resize_height,
                image::imageops::FilterType::Nearest,
            );
        }
    }

    // STEP 2: Apply brightness adjustment
    if config.enable_brightness && config.brightness_boost != 0 {
        processed = adjust_brightness(&processed, config.brightness_boost);
    }

    // STEP 3: Apply contrast boost
    if config.enable_contrast && config.contrast_boost != 0.0 {
        processed = adjust_contrast(&processed, config.contrast_boost);
    }

    processed
}

/// Adjust image brightness
fn adjust_brightness(image: &DynamicImage, adjustment: i32) -> DynamicImage {
    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8();

    let adjusted = ImageBuffer::from_fn(width, height, |x, y| {
        let pixel = rgba.get_pixel(x, y);
        Rgba([
            clamp_u8(pixel[0] as i32 + adjustment),
            clamp_u8(pixel[1] as i32 + adjustment),
            clamp_u8(pixel[2] as i32 + adjustment),
            pixel[3], // Keep alpha unchanged
        ])
    });

    DynamicImage::ImageRgba8(adjusted)
}

/// Adjust image contrast
/// Formula: new_value = ((old_value - 128) * factor) + 128
fn adjust_contrast(image: &DynamicImage, boost: f32) -> DynamicImage {
    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8();

    // Convert boost to contrast factor (boost of 10 = factor of ~1.1)
    let factor = 1.0 + (boost / 100.0);

    let adjusted = ImageBuffer::from_fn(width, height, |x, y| {
        let pixel = rgba.get_pixel(x, y);
        Rgba([
            clamp_u8((((pixel[0] as f32 - 128.0) * factor) + 128.0) as i32),
            clamp_u8((((pixel[1] as f32 - 128.0) * factor) + 128.0) as i32),
            clamp_u8((((pixel[2] as f32 - 128.0) * factor) + 128.0) as i32),
            pixel[3], // Keep alpha unchanged
        ])
    });

    DynamicImage::ImageRgba8(adjusted)
}

/// Clamp value to valid u8 range [0, 255]
fn clamp_u8(value: i32) -> u8 {
    value.max(0).min(255) as u8
}

/// Quick resize for non-critical images (uses faster Nearest filter)
pub fn quick_resize(image: &DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    let (width, height) = image.dimensions();

    if width <= max_width && height <= max_height {
        return image.clone();
    }

    let aspect = width as f32 / height as f32;
    let (new_width, new_height) = if aspect > 1.0 {
        (max_width, (max_width as f32 / aspect) as u32)
    } else {
        ((max_height as f32 * aspect) as u32, max_height)
    };

    image.resize(new_width, new_height, image::imageops::FilterType::Nearest)
}

/// Calculate optimal dimensions that fit within max bounds while preserving aspect ratio
pub fn calculate_fit_dimensions(
    original_width: u32,
    original_height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    if original_width <= max_width && original_height <= max_height {
        return (original_width, original_height);
    }

    let aspect = original_width as f32 / original_height as f32;
    let target_aspect = max_width as f32 / max_height as f32;

    if aspect > target_aspect {
        // Fit to width
        (max_width, (max_width as f32 / aspect) as u32)
    } else {
        // Fit to height
        ((max_height as f32 * aspect) as u32, max_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_u8() {
        assert_eq!(clamp_u8(-10), 0);
        assert_eq!(clamp_u8(0), 0);
        assert_eq!(clamp_u8(128), 128);
        assert_eq!(clamp_u8(255), 255);
        assert_eq!(clamp_u8(300), 255);
    }

    #[test]
    fn test_calculate_fit_dimensions_wider() {
        // Image wider than target (16:9 → 16:9 target)
        let (w, h) = calculate_fit_dimensions(1920, 1080, 1280, 720);
        assert_eq!(w, 1280);
        assert_eq!(h, 720);
    }

    #[test]
    fn test_calculate_fit_dimensions_taller() {
        // Image taller than target (9:16 → 16:9 target)
        let (w, h) = calculate_fit_dimensions(1080, 1920, 1280, 720);
        assert_eq!(w, 405);
        assert_eq!(h, 720);
    }

    #[test]
    fn test_calculate_fit_dimensions_already_fits() {
        // Image already smaller than target
        let (w, h) = calculate_fit_dimensions(800, 600, 1280, 720);
        assert_eq!(w, 800);
        assert_eq!(h, 600);
    }

    #[test]
    fn test_preprocess_config_default() {
        let config = PreprocessConfig::default();
        assert_eq!(config.target_width, 1280);
        assert_eq!(config.target_height, 720);
        assert_eq!(config.contrast_boost, 10.0);
        assert_eq!(config.brightness_boost, 5);
        assert!(config.enable_resize);
        assert!(config.enable_contrast);
        assert!(config.enable_brightness);
    }

    #[test]
    fn test_brightness_adjustment() {
        // Create a simple 2x2 gray image
        let img = DynamicImage::ImageRgba8(ImageBuffer::from_fn(2, 2, |_, _| {
            Rgba([100, 100, 100, 255])
        }));

        let brightened = adjust_brightness(&img, 50);
        let pixel = brightened.to_rgba8().get_pixel(0, 0);

        assert_eq!(pixel[0], 150);
        assert_eq!(pixel[1], 150);
        assert_eq!(pixel[2], 150);
        assert_eq!(pixel[3], 255); // Alpha unchanged
    }

    #[test]
    fn test_brightness_clamping() {
        // Create a bright image
        let img = DynamicImage::ImageRgba8(ImageBuffer::from_fn(2, 2, |_, _| {
            Rgba([250, 250, 250, 255])
        }));

        let brightened = adjust_brightness(&img, 50);
        let pixel = brightened.to_rgba8().get_pixel(0, 0);

        // Should clamp to 255
        assert_eq!(pixel[0], 255);
        assert_eq!(pixel[1], 255);
        assert_eq!(pixel[2], 255);
    }
}
