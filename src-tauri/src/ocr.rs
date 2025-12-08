// src-tauri/src/ocr.rs

use image::DynamicImage;
use std::process::Command;
use std::fs;

/// Extract text from an image using Tesseract OCR
pub fn extract_text_from_image(img: &DynamicImage) -> Result<String, String> {
    // Save image to temp file
    let temp_path = std::env::temp_dir().join("pkr_ocr_temp.png");
    img.save(&temp_path)
        .map_err(|e| format!("Failed to save temp image: {}", e))?;
    
    // Run tesseract command
    let output = Command::new("tesseract")
        .arg(&temp_path)
        .arg("stdout")
        .arg("--psm")
        .arg("6")
        .output()
        .map_err(|e| format!("Failed to run tesseract: {}", e))?;
    
    // Clean up temp file
    let _ = fs::remove_file(&temp_path);
    
    if !output.status.success() {
        return Err(format!("Tesseract failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(text)
}

/// Extract text from a base64-encoded PNG image
pub fn extract_text_from_base64(base64_img: &str) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose};
    
    // Decode base64
    let img_bytes = general_purpose::STANDARD
        .decode(base64_img)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;
    
    // Load image
    let img = image::load_from_memory(&img_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;
    
    // Extract text
    extract_text_from_image(&img)
}

/// Parse poker-specific data from OCR text
pub fn parse_poker_data(ocr_text: &str) -> PokerData {
    let text = ocr_text.to_uppercase();
    
    PokerData {
        raw_text: ocr_text.to_string(),
        cards_detected: extract_cards(&text),
        pot_size: extract_pot(&text),
        position: extract_position(&text),
    }
}

#[derive(Debug, Clone)]
pub struct PokerData {
    pub raw_text: String,
    pub cards_detected: Vec<String>,
    pub pot_size: Option<f64>,
    pub position: Option<String>,
}

/// Extract card notations from text (e.g., "AS", "KH", "QD")
pub fn extract_cards(text: &str) -> Vec<String> {
    let mut cards = Vec::new();
    let ranks = vec!["A", "K", "Q", "J", "T", "9", "8", "7", "6", "5", "4", "3", "2"];
    let suits = vec!["S", "H", "D", "C", "♠", "♥", "♦", "♣"];
    
    for rank in &ranks {
        for suit in &suits {
            let card = format!("{}{}", rank, suit);
            if text.contains(&card) {
                cards.push(card);
            }
        }
    }
    
    cards
}

/// Extract pot size from text (looks for dollar amounts)
pub fn extract_pot(text: &str) -> Option<f64> {
    // Look for patterns like "$123.45" or "POT: $123"
    let re = regex::Regex::new(r"\$\s*(\d+(?:\.\d{2})?)").ok()?;
    
    for cap in re.captures_iter(text) {
        if let Some(amount) = cap.get(1) {
            if let Ok(value) = amount.as_str().parse::<f64>() {
                return Some(value);
            }
        }
    }
    
    None
}

/// Extract position from text (BTN, SB, BB, etc.)
pub fn extract_position(text: &str) -> Option<String> {
    let positions = vec!["BTN", "SB", "BB", "UTG", "MP", "CO"];
    
    for pos in positions {
        if text.contains(pos) {
            return Some(pos.to_string());
        }
    }
    
    None
}