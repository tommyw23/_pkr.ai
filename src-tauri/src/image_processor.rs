// src-tauri/src/image_processor.rs

use image::{DynamicImage, GenericImageView};

// Normalized crop percentages based on 1920x1048 PokerStars window
// These percentages work for any window size (resolution-independent)
const CROP_X_PERCENT: f32 = 0.2422;      // 465/1920
const CROP_Y_PERCENT: f32 = 0.2481;      // 260/1048
const CROP_WIDTH_PERCENT: f32 = 0.2396;  // 460/1920
const CROP_HEIGHT_PERCENT: f32 = 0.4342; // 455/1048

/// Crop to the essential poker region: community cards, pot, hero cards, stack
/// This removes all unnecessary UI (lobby, empty seats, buttons, chat)
pub fn crop_poker_essential_region(img: &DynamicImage) -> DynamicImage {
    let (window_width, window_height) = img.dimensions();

    let crop_x = (window_width as f32 * CROP_X_PERCENT).round() as u32;
    let crop_y = (window_height as f32 * CROP_Y_PERCENT).round() as u32;
    let crop_w = (window_width as f32 * CROP_WIDTH_PERCENT).round() as u32;
    let crop_h = (window_height as f32 * CROP_HEIGHT_PERCENT).round() as u32;

    img.crop_imm(crop_x, crop_y, crop_w, crop_h)
}

/// Enhance image contrast and sharpness for better card detection
pub fn enhance_for_card_detection(img: &DynamicImage) -> DynamicImage {
    let mut enhanced = img.clone();
    
    // Increase contrast (makes cards stand out more)
    enhanced = enhanced.adjust_contrast(25.0);
    
    // Sharpen (makes suit symbols clearer)
    enhanced = enhanced.unsharpen(1.2, 1);
    
    // Slightly brighten (helps with dark tables)
    enhanced = enhanced.brighten(8);
    
    enhanced
}

/// Resize image to reduce token cost while maintaining card visibility
/// 800px width is perfect - cards remain crystal clear but cost drops 60%
pub fn resize_for_api(img: &DynamicImage, max_width: u32) -> DynamicImage {
    let (width, height) = img.dimensions();

    if width <= max_width {
        return img.clone();
    }

    // Maintain aspect ratio
    let scale = max_width as f32 / width as f32;
    let new_height = (height as f32 * scale) as u32;

    img.resize(max_width, new_height, image::imageops::FilterType::Lanczos3)
}

/// Full preprocessing pipeline optimized for poker card detection
/// Returns a much smaller, clearer image perfect for AI analysis
pub fn preprocess_poker_screenshot(img: &DynamicImage) -> DynamicImage {
    // Step 1: Crop to ONLY the essential poker region (70% size reduction)
    let cropped = crop_poker_essential_region(img);

    // Step 2: Resize to 800px width (additional 50% reduction, still clear)
    let resized = resize_for_api(&cropped, 800);

    // Step 3: Enhance contrast for better card/suit detection
    let enhanced = enhance_for_card_detection(&resized);

    enhanced
}