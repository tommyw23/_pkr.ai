// src-tauri/src/poker_capture.rs

use screenshots::Screen;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use tauri::{AppHandle, Emitter, Manager};
use once_cell::sync::Lazy;

// Global state tracking for cascade inference
static PREVIOUS_STATE: Lazy<Mutex<Option<crate::poker_types::PokerState>>> = 
    Lazy::new(|| Mutex::new(None));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PokerRegions {
    pub hole_cards: String,
    pub community_cards: String,
    pub pot: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParsedPokerData {
    pub your_cards: Vec<String>,
    pub community_cards: Vec<String>,
    pub pot_size: Option<f64>,
    pub position: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PokerWindow {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapturedGameState {
    pub image_base64: String,
    pub timestamp: u64,
    pub window_title: String,
    pub ocr_text: Option<String>,
    pub cards_detected: Vec<String>,
    pub pot_size: Option<f64>,
    pub position: Option<String>,
}

pub struct MonitoringState {
    pub is_running: Arc<Mutex<bool>>,
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self {
            is_running: Arc::new(Mutex::new(false)),
        }
    }
}

/// Detect if a new hand has started (pot reset)
fn detect_new_hand(current_state: &crate::poker_types::PokerState) -> bool {
    let prev_state_clone = {
        let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
        prev_state_guard.clone()
    }; // Lock dropped here
    
    if let Some(prev) = prev_state_clone {
        // New hand detected if:
        // 1. Pot resets from high to low (hand ended)
        // 2. OR board cards disappear (new deal)
        
        if let (Some(prev_pot), Some(curr_pot)) = (prev.pot_size, current_state.pot_size) {
            // Pot reset: was high (>$2000), now low (<$1000)
            if prev_pot > 2000.0 && curr_pot < 1000.0 {
                return true;
            }
        }
        
        // Board cards reset (5 cards -> 0 cards)
        if prev.board_cards.len() >= 3 && current_state.board_cards.is_empty() {
            return true;
        }
    }
    
    false
}

/// Capture poker window and analyze with CASCADE INFERENCE
pub async fn capture_poker_regions(window_title: String) -> Result<ParsedPokerData, String> {
    let windows = find_poker_windows().await?;
    let poker_window = windows.iter()
        .find(|w| w.title == window_title)
        .ok_or("Poker window not found")?;

    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens.first().ok_or("No screens found")?;
    let full_image = screen.capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    let img_buffer = image::RgbaImage::from_raw(
        full_image.width(),
        full_image.height(),
        full_image.rgba().to_vec(),
    ).ok_or("Failed to create image buffer")?;
    
    let dynamic_img = image::DynamicImage::ImageRgba8(img_buffer);
    
    let crop_x = poker_window.x.max(0) as u32;
    let crop_y = poker_window.y.max(0) as u32;
    let crop_width = poker_window.width.min(dynamic_img.width() - crop_x);
    let crop_height = poker_window.height.min(dynamic_img.height() - crop_y);
    
    let window_img = dynamic_img.crop_imm(crop_x, crop_y, crop_width, crop_height);

    println!("🎯 Original poker window: {}x{}", window_img.width(), window_img.height());

    // Preprocess: crop to essential region, resize, enhance
    let processed_img = crate::image_processor::preprocess_poker_screenshot(&window_img);

    // Convert processed image to PNG bytes
    let mut png_bytes = Vec::new();
    processed_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    let size_kb = png_bytes.len() as f32 / 1024.0;
    println!("📦 Final image size: {:.1} KB ({}x{})", size_kb, processed_img.width(), processed_img.height());
    
    println!("🤖 Step 1: Analyzing with Gemini (fast)...");
    
    // STEP 1: Try Gemini first (cheap and fast)
    let gemini_result = match crate::gemini::analyze_poker_screenshot(&png_bytes).await {
        Ok(result) => Some(result),
        Err(e) => {
            if e.contains("429") || e.contains("RESOURCE_EXHAUSTED") {
                println!("⚠️  Gemini rate limit hit! Skipping to Claude...");
                None
            } else {
                return Err(e);
            }
        }
    };

    let final_state = if let Some(gemini_result) = gemini_result {
        // STEP 2: Validate Gemini's output
        let validation = crate::validator::validate_poker_state(&gemini_result);
        
        if validation.is_valid && gemini_result.overall_confidence >= 0.85 {
            println!("✅ Gemini output is valid! (confidence: {:.2})", gemini_result.overall_confidence);
            gemini_result
        } else {
            println!("⚠️  Gemini has issues: {:?}", validation.issues);
            println!("🔄 Step 2: Escalating to Claude (accurate)...");
            
            // STEP 3: Escalate to Claude for correction
            let gemini_json = serde_json::to_string_pretty(&gemini_result)
                .unwrap_or_else(|_| "{}".to_string());
            
            // Clone previous state and drop the lock BEFORE await
            let prev_state_clone = {
                let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
                prev_state_guard.clone()
            }; // Lock dropped here
            
            let claude_result = crate::claude_vision::analyze_with_claude(
                &png_bytes,
                prev_state_clone.as_ref(),
                &gemini_json,
                &validation.issues,
            ).await?;
            
            println!("✅ Claude corrected output! (confidence: {:.2})", claude_result.overall_confidence);
            claude_result
        }
    } else {
        // Gemini unavailable - go straight to Claude
        println!("🤖 Using Claude directly...");
        
        // Clone previous state and drop the lock BEFORE await
        let prev_state_clone = {
            let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
            prev_state_guard.clone()
        }; // Lock dropped here
        
        let claude_result = crate::claude_vision::analyze_with_claude(
            &png_bytes,
            prev_state_clone.as_ref(),
            "{}",
            &["gemini_unavailable".to_string()],
        ).await?;
        
        println!("✅ Claude analysis complete! (confidence: {:.2})", claude_result.overall_confidence);
        claude_result
    };
    
    // Check if this is a new hand
    if detect_new_hand(&final_state) {
        println!("🆕 NEW HAND DETECTED! Clearing previous state.");
        *PREVIOUS_STATE.lock().unwrap() = None;
    }
    
    // Save current state for next iteration
    *PREVIOUS_STATE.lock().unwrap() = Some(final_state.clone());
    
    // Convert to display format
    let your_cards = crate::poker_types::PokerState::to_display_cards(&final_state.hero_cards);
    let community_cards = crate::poker_types::PokerState::to_display_cards(&final_state.board_cards);
    
    println!("🃏 Your cards: {:?}", your_cards);
    println!("🎴 Community: {:?}", community_cards);
    if let Some(pot) = final_state.pot_size {
        println!("💰 Pot: ${}", pot);
    }
    if let Some(ref pos) = final_state.hero_position {
        println!("📍 Position: {}", pos);
    }
    println!("📊 Overall confidence: {:.1}%", final_state.overall_confidence * 100.0);
    
    Ok(ParsedPokerData {
        your_cards,
        community_cards,
        pot_size: final_state.pot_size,
        position: final_state.hero_position,
    })
}

#[tauri::command]
pub async fn find_poker_windows() -> Result<Vec<PokerWindow>, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowRect, IsWindowVisible,
        };
        use std::sync::Mutex;

        let windows: Mutex<Vec<PokerWindow>> = Mutex::new(Vec::new());

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let windows_ptr = lparam.0 as *const Mutex<Vec<PokerWindow>>;
            let windows = &*windows_ptr;

            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }

            let mut title: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, &mut title);
            
            if len == 0 {
                return BOOL(1);
            }

            let title_str = String::from_utf16_lossy(&title[..len as usize]);

            let poker_keywords = [
                "PokerStars", 
                "GGPoker", 
                "888poker", 
                "partypoker",
                "ACR",
                "BetOnline",
                "Ignition",
                "Bovada",
                "WSOP",
                "Replay Poker",
                "Global Poker",
                "poker",
            ];

            let is_poker = poker_keywords.iter().any(|&kw| title_str.contains(kw));

            if !is_poker {
                return BOOL(1);
            }

            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                let window = PokerWindow {
                    title: title_str,
                    x: rect.left,
                    y: rect.top,
                    width: (rect.right - rect.left) as u32,
                    height: (rect.bottom - rect.top) as u32,
                };

                if let Ok(mut vec) = windows.lock() {
                    vec.push(window);
                }
            }

            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(
                Some(enum_callback),
                LPARAM(&windows as *const _ as isize),
            );
        }

        Ok(windows.into_inner().unwrap())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn capture_poker_window(window_title: String) -> Result<CapturedGameState, String> {
    let windows = find_poker_windows().await?;
    let poker_window = windows.iter()
        .find(|w| w.title == window_title)
        .ok_or("Poker window not found")?;
    
    println!("📐 Window bounds: x={}, y={}, w={}, h={}", 
        poker_window.x, poker_window.y, poker_window.width, poker_window.height);

    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens.first().ok_or("No screens found")?;
    let full_image = screen.capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    let img_buffer = image::RgbaImage::from_raw(
        full_image.width(),
        full_image.height(),
        full_image.rgba().to_vec(),
    ).ok_or("Failed to create image buffer")?;
    
    let dynamic_img = image::DynamicImage::ImageRgba8(img_buffer);
    
    let crop_x = poker_window.x.max(0) as u32;
    let crop_y = poker_window.y.max(0) as u32;
    let crop_width = poker_window.width.min(dynamic_img.width() - crop_x);
    let crop_height = poker_window.height.min(dynamic_img.height() - crop_y);
    
    println!("✂️  Cropping to: x={}, y={}, w={}, h={}", crop_x, crop_y, crop_width, crop_height);
    
    let cropped_img = dynamic_img.crop_imm(crop_x, crop_y, crop_width, crop_height);

    let mut png_bytes = Vec::new();
    cropped_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    
    let base64_image = general_purpose::STANDARD.encode(&png_bytes);

    Ok(CapturedGameState {
        image_base64: base64_image,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        window_title,
        ocr_text: None,
        cards_detected: vec![],
        pot_size: None,
        position: None,
    })
}

#[tauri::command]
pub async fn start_poker_monitoring(
    app: AppHandle,
    state: tauri::State<'_, MonitoringState>,
) -> Result<(), String> {
    {
        let mut is_running = state.is_running.lock().unwrap();
        if *is_running {
            return Ok(());
        }
        *is_running = true;
    }

    println!("Starting poker monitoring background task...");

    let is_running = Arc::clone(&state.is_running);
    
    tauri::async_runtime::spawn(async move {
        let mut capture_count = 0;
        
        while *is_running.lock().unwrap() {
            capture_count += 1;
            println!("📸 Capture #{}: Taking screenshot...", capture_count);
            
            match find_poker_windows().await {
                Ok(windows) => {
                    if windows.is_empty() {
                        println!("⚠️  No poker windows found");
                    } else {
                        let window = &windows[0];
                        println!("🎯 Found: {}", window.title);
                        
                        match capture_poker_regions(window.title.clone()).await {
                            Ok(parsed_data) => {
                                println!("✅ Analysis complete!");
                                println!("🃏 Your cards: {:?}", parsed_data.your_cards);
                                println!("🎴 Community: {:?}", parsed_data.community_cards);
                                if let Some(pot) = parsed_data.pot_size {
                                    println!("💰 Pot: ${}", pot);
                                }
                                if let Some(ref pos) = parsed_data.position {
                                    println!("📍 Position: {}", pos);
                                }
                                
                                let _ = app.emit("poker-capture", &parsed_data);
                            }
                            Err(e) => {
                                println!("❌ Capture error: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Window detection error: {}", e);
                }
            }
            
            sleep(Duration::from_secs(2)).await;
        }
        
        println!("🛑 Monitoring stopped");
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_poker_monitoring(
    state: tauri::State<'_, MonitoringState>,
) -> Result<(), String> {
    println!("Stopping poker monitoring...");
    let mut is_running = state.is_running.lock().unwrap();
    *is_running = false;
    
    // Clear previous state when stopping
    *PREVIOUS_STATE.lock().unwrap() = None;
    
    Ok(())
}

#[tauri::command]
pub async fn capture_poker_region(
    _x: i32,
    _y: i32,
    _width: u32,
    _height: u32,
) -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    
    let screen = screens
        .first()
        .ok_or("No screens found")?;

    let image = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;
    
    let png_bytes = image
        .to_png()
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(general_purpose::STANDARD.encode(&png_bytes))
}