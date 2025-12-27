// src-tauri/src/vision/frame_processor.rs
// Frame filtering pipeline to skip unchanged frames before calling vision APIs

use image::DynamicImage;
use std::sync::Mutex;
use once_cell::sync::Lazy;

/// Global state to track previous frame for comparison
static PREVIOUS_FRAME: Lazy<Mutex<Option<FrameState>>> =
    Lazy::new(|| Mutex::new(None));

/// Global statistics for frame filtering
static FRAME_STATS: Lazy<Mutex<FrameStatistics>> =
    Lazy::new(|| Mutex::new(FrameStatistics::default()));

#[derive(Clone, Debug)]
struct FrameState {
    hash: u64,
    pixel_checksum: u64,
    green_pixel_ratio: f32,
    timestamp: std::time::Instant,
}

#[derive(Debug, Clone, Default)]
pub struct FrameStatistics {
    pub total_frames: u64,
    pub processed_frames: u64,
    pub skipped_frames: u64,
    pub skipped_low_change: u64,
    pub skipped_no_green: u64,
    pub api_calls_saved: u64,
}

impl FrameStatistics {
    pub fn cost_savings_estimate(&self) -> f64 {
        // Estimate cost savings based on API calls avoided
        // Gemini Flash: ~$0.00001875 per image (1.5M tokens for $0.075/1M)
        // Claude Sonnet: ~$0.015 per image (assuming 1k tokens @ $15/1M output)
        // Average: ~$0.0075 per call saved (conservative estimate)
        self.api_calls_saved as f64 * 0.0075
    }

    pub fn skip_rate(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.skipped_frames as f64 / self.total_frames as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameFilterResult {
    pub should_process: bool,
    pub reason: String,
    pub diff_percentage: f32,
    pub green_felt_detected: bool,
}

/// Configuration for frame filtering
#[derive(Debug, Clone)]
pub struct FrameFilterConfig {
    /// Minimum difference threshold (0.0 to 1.0)
    /// Frames with less than this % change will be skipped
    pub min_diff_threshold: f32,

    /// Minimum green pixel ratio (0.0 to 1.0)
    /// Frames below this ratio are likely not poker tables
    pub min_green_ratio: f32,

    /// Maximum time between forced processing (seconds)
    /// Even if frame is unchanged, process after this timeout
    pub max_skip_duration_secs: u64,

    /// Enable perceptual hashing
    pub use_perceptual_hash: bool,
}

impl Default for FrameFilterConfig {
    fn default() -> Self {
        Self {
            min_diff_threshold: 0.02,  // 2% change threshold
            min_green_ratio: 0.015,     // 1.5% green pixels minimum (supports darker felts like Ignition)
            max_skip_duration_secs: 5,  // Force process every 5 seconds (matches capture interval)
            use_perceptual_hash: true,
        }
    }
}

/// Main frame filtering function
/// Returns whether the frame should be processed by vision APIs
pub fn should_process_frame(
    frame: &DynamicImage,
    config: &FrameFilterConfig,
) -> FrameFilterResult {
    let start = std::time::Instant::now();

    // Calculate frame metrics
    let pixel_checksum = calculate_pixel_checksum(frame);
    let green_ratio = calculate_green_felt_ratio(frame);

    let hash = if config.use_perceptual_hash {
        calculate_perceptual_hash(frame)
    } else {
        0
    };

    // Update statistics
    {
        let mut stats = FRAME_STATS.lock().unwrap();
        stats.total_frames += 1;
    }

    // Check green felt heuristic first (cheapest check)
    if green_ratio < config.min_green_ratio {
        // Update statistics
        let mut stats = FRAME_STATS.lock().unwrap();
        stats.skipped_frames += 1;
        stats.skipped_no_green += 1;
        stats.api_calls_saved += 1;

        return FrameFilterResult {
            should_process: false,
            reason: format!("Low green ratio: {:.1}%", green_ratio * 100.0),
            diff_percentage: 0.0,
            green_felt_detected: false,
        };
    }

    // Get previous frame state
    let mut prev_state_guard = PREVIOUS_FRAME.lock().unwrap();

    // Clone previous state for comparison (if it exists)
    let prev_state = prev_state_guard.as_ref().map(|state| state.clone());

    // UPDATE TIMESTAMP IMMEDIATELY - this happens on EVERY call, whether frame is processed or skipped
    // This prevents elapsed time from accumulating incorrectly
    *prev_state_guard = Some(FrameState {
        hash,
        pixel_checksum,
        green_pixel_ratio: green_ratio,
        timestamp: start,
    });

    // If no previous frame, process this one (first frame)
    if prev_state.is_none() {
        // Update statistics
        let mut stats = FRAME_STATS.lock().unwrap();
        stats.processed_frames += 1;

        return FrameFilterResult {
            should_process: true,
            reason: "First frame".to_string(),
            diff_percentage: 100.0,
            green_felt_detected: true,
        };
    }

    let prev_state = prev_state.unwrap();

    // Check if max skip duration exceeded
    let elapsed = start.duration_since(prev_state.timestamp).as_secs();
    if elapsed >= config.max_skip_duration_secs {
        // Update statistics
        let mut stats = FRAME_STATS.lock().unwrap();
        stats.processed_frames += 1;

        return FrameFilterResult {
            should_process: true,
            reason: format!("Timeout: {}s elapsed", elapsed),
            diff_percentage: 0.0,
            green_felt_detected: true,
        };
    }

    // Calculate difference percentage
    let diff_percentage = if config.use_perceptual_hash {
        calculate_hash_difference(prev_state.hash, hash)
    } else {
        calculate_checksum_difference(prev_state.pixel_checksum, pixel_checksum)
    };

    // Decide whether to process
    let should_process = diff_percentage >= config.min_diff_threshold;

    if should_process {
        // Update statistics
        let mut stats = FRAME_STATS.lock().unwrap();
        stats.processed_frames += 1;

        FrameFilterResult {
            should_process: true,
            reason: format!("Changed: {:.1}%", diff_percentage * 100.0),
            diff_percentage,
            green_felt_detected: true,
        }
    } else {
        // Update statistics
        let mut stats = FRAME_STATS.lock().unwrap();
        stats.skipped_frames += 1;
        stats.skipped_low_change += 1;
        stats.api_calls_saved += 1;

        FrameFilterResult {
            should_process: false,
            reason: format!("Low change: {:.1}%", diff_percentage * 100.0),
            diff_percentage,
            green_felt_detected: true,
        }
    }
}

/// Reset the previous frame state (call when starting new monitoring session)
pub fn reset_frame_state() {
    *PREVIOUS_FRAME.lock().unwrap() = None;
}

/// Get current frame filtering statistics
pub fn get_frame_statistics() -> FrameStatistics {
    FRAME_STATS.lock().unwrap().clone()
}

/// Reset frame filtering statistics
pub fn reset_frame_statistics() {
    *FRAME_STATS.lock().unwrap() = FrameStatistics::default();
}

/// Print frame filtering statistics summary
pub fn print_frame_statistics() {
    let stats = get_frame_statistics();
    println!("\n📊 Frame Filtering Statistics:");
    println!("   Total frames: {}", stats.total_frames);
    println!("   Processed: {} ({:.1}%)",
        stats.processed_frames,
        (stats.processed_frames as f64 / stats.total_frames.max(1) as f64) * 100.0
    );
    println!("   Skipped: {} ({:.1}%)",
        stats.skipped_frames,
        stats.skip_rate() * 100.0
    );
    println!("     - Low change: {}", stats.skipped_low_change);
    println!("     - No green felt: {}", stats.skipped_no_green);
    println!("   API calls saved: {}", stats.api_calls_saved);
    println!("   Estimated cost savings: ${:.4}", stats.cost_savings_estimate());
    println!();
}

/// Calculate a simple pixel checksum for fast comparison
fn calculate_pixel_checksum(frame: &DynamicImage) -> u64 {
    // Downsample to 32x32 for speed
    let small = frame.resize_exact(32, 32, image::imageops::FilterType::Nearest);
    let rgba = small.to_rgba8();

    let mut checksum: u64 = 0;
    for (i, pixel) in rgba.pixels().enumerate() {
        // Weight by position to detect spatial changes
        let weight = (i as u64 + 1) % 997; // Use prime for better distribution
        checksum = checksum.wrapping_add(
            (pixel[0] as u64 * weight)
                .wrapping_add(pixel[1] as u64 * weight)
                .wrapping_add(pixel[2] as u64 * weight)
        );
    }

    checksum
}

/// Calculate perceptual hash (pHash-like algorithm)
/// Returns a 64-bit hash where similar images have similar hashes
fn calculate_perceptual_hash(frame: &DynamicImage) -> u64 {
    // Resize to 8x8 grayscale - use NEAREST filter for speed (Lanczos3 is 100x slower for 8x8!)
    // For perceptual hash, we only need rough structure, not high-quality interpolation
    let small = frame.resize_exact(8, 8, image::imageops::FilterType::Nearest);
    let gray = small.to_luma8();

    // Calculate average pixel value
    let sum: u32 = gray.pixels().map(|p| p[0] as u32).sum();
    let avg: u32 = sum / 64;

    // Create hash: bit is 1 if pixel > average, 0 otherwise
    let mut hash: u64 = 0;
    for (i, pixel) in gray.pixels().enumerate() {
        if pixel[0] as u32 > avg {
            hash |= 1 << i;
        }
    }

    hash
}

/// Calculate green felt ratio (poker table heuristic)
/// Most poker tables have significant green/teal coloring
fn calculate_green_felt_ratio(frame: &DynamicImage) -> f32 {
    // Downsample for speed
    let small = frame.resize_exact(64, 64, image::imageops::FilterType::Nearest);
    let rgba = small.to_rgba8();

    let mut green_pixels = 0;
    let total_pixels = rgba.pixels().len() as u32;

    for pixel in rgba.pixels() {
        let r = pixel[0] as i32;
        let g = pixel[1] as i32;
        let b = pixel[2] as i32;

        // Detect green/teal colors typical of poker tables
        // Green should be dominant, red and blue lower
        let is_green = g > 60 && g > r && g > b && (g - r) > 20;

        // Also detect teal/cyan (common in modern poker clients)
        let is_teal = g > 60 && b > 60 && g > r && b > r;

        if is_green || is_teal {
            green_pixels += 1;
        }
    }

    green_pixels as f32 / total_pixels as f32
}

/// Calculate difference between two perceptual hashes
/// Returns a value between 0.0 (identical) and 1.0 (completely different)
fn calculate_hash_difference(hash1: u64, hash2: u64) -> f32 {
    let xor = hash1 ^ hash2;
    let hamming_distance = xor.count_ones() as f32;
    hamming_distance / 64.0
}

/// Calculate difference between two pixel checksums
/// Returns a normalized value between 0.0 and 1.0
fn calculate_checksum_difference(checksum1: u64, checksum2: u64) -> f32 {
    let diff = checksum1.abs_diff(checksum2) as f32;
    // Normalize to 0-1 range (max possible diff for 32x32 image)
    let max_diff = (32.0 * 32.0 * 255.0 * 3.0) as f32;
    (diff / max_diff).min(1.0)
}

/// Helper to check if frame is likely a poker table
pub fn is_likely_poker_table(frame: &DynamicImage) -> bool {
    let green_ratio = calculate_green_felt_ratio(frame);
    green_ratio >= 0.05 // 5% threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn create_test_image(width: u32, height: u32, r: u8, g: u8, b: u8) -> DynamicImage {
        let img = RgbaImage::from_fn(width, height, |_, _| {
            image::Rgba([r, g, b, 255])
        });
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn test_green_felt_detection() {
        // Green image (poker table)
        let green_img = create_test_image(100, 100, 50, 150, 50);
        let green_ratio = calculate_green_felt_ratio(&green_img);
        assert!(green_ratio > 0.5, "Green image should have high green ratio");

        // Red image (not poker table)
        let red_img = create_test_image(100, 100, 150, 50, 50);
        let red_ratio = calculate_green_felt_ratio(&red_img);
        assert!(red_ratio < 0.1, "Red image should have low green ratio");
    }

    #[test]
    fn test_perceptual_hash_similar() {
        let img1 = create_test_image(100, 100, 100, 100, 100);
        let img2 = create_test_image(100, 100, 105, 105, 105);

        let hash1 = calculate_perceptual_hash(&img1);
        let hash2 = calculate_perceptual_hash(&img2);

        let diff = calculate_hash_difference(hash1, hash2);
        assert!(diff < 0.2, "Similar images should have low hash difference");
    }

    #[test]
    fn test_perceptual_hash_different() {
        let img1 = create_test_image(100, 100, 50, 50, 50);
        let img2 = create_test_image(100, 100, 200, 200, 200);

        let hash1 = calculate_perceptual_hash(&img1);
        let hash2 = calculate_perceptual_hash(&img2);

        let diff = calculate_hash_difference(hash1, hash2);
        assert!(diff > 0.4, "Different images should have high hash difference");
    }

    #[test]
    fn test_first_frame_always_processes() {
        reset_frame_state();

        let img = create_test_image(100, 100, 50, 150, 50);
        let config = FrameFilterConfig::default();
        let result = should_process_frame(&img, &config);

        assert!(result.should_process, "First frame should always process");
        assert_eq!(result.reason, "First frame");
    }

    #[test]
    fn test_identical_frame_skipped() {
        reset_frame_state();

        let img = create_test_image(100, 100, 50, 150, 50);
        let config = FrameFilterConfig::default();

        // Process first frame
        let result1 = should_process_frame(&img, &config);
        assert!(result1.should_process);

        // Try same frame again
        let result2 = should_process_frame(&img, &config);
        assert!(!result2.should_process, "Identical frame should be skipped");
    }

    #[test]
    fn test_low_green_ratio_filtered() {
        reset_frame_state();

        // Create non-green image (not a poker table)
        let img = create_test_image(100, 100, 150, 50, 50);
        let config = FrameFilterConfig::default();

        let result = should_process_frame(&img, &config);
        assert!(!result.should_process, "Low green ratio should be filtered");
        assert!(!result.green_felt_detected);
    }
}
