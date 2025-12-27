use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::{thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use xcap::Monitor;

// Global state to store current calibration session's monitor info
static CURRENT_CALIBRATION_MONITOR: Mutex<Option<MonitorInfo>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRegion {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MonitorInfo {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationData {
    pub regions: Vec<CalibrationRegion>,
    pub window_width: u32,
    pub window_height: u32,
    #[serde(default)]
    pub monitor: Option<MonitorInfo>,
}

fn get_calibration_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    Ok(app_data_dir.join("calibration.json"))
}

#[tauri::command]
pub async fn start_calibration(app: AppHandle) -> Result<MonitorInfo, String> {
    // Get the main window to determine which monitor it's on
    let main_window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    // Get the main window's position
    let window_pos = main_window
        .outer_position()
        .map_err(|e| format!("Failed to get window position: {}", e))?;

    let window_center_x = window_pos.x + 280; // Approximate center (560/2)
    let window_center_y = window_pos.y + 40; // Approximate center

    // Get all monitors
    let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;

    if monitors.is_empty() {
        return Err("No monitors found".to_string());
    }

    // Find the monitor that contains the main window's center
    let target_monitor = monitors
        .iter()
        .find(|m| {
            let mx = m.x();
            let my = m.y();
            let mw = m.width() as i32;
            let mh = m.height() as i32;

            window_center_x >= mx
                && window_center_x < mx + mw
                && window_center_y >= my
                && window_center_y < my + mh
        })
        .or_else(|| monitors.iter().find(|m| m.is_primary()))
        .ok_or("Could not find a suitable monitor")?;

    let monitor_x = target_monitor.x();
    let monitor_y = target_monitor.y();
    let monitor_width = target_monitor.width();
    let monitor_height = target_monitor.height();
    let scale_factor = target_monitor.scale_factor();

    let monitor_info = MonitorInfo {
        x: monitor_x,
        y: monitor_y,
        width: monitor_width,
        height: monitor_height,
        scale_factor: scale_factor as f64,
    };

    // Store monitor info for later retrieval by CalibrationOverlay
    if let Ok(mut guard) = CURRENT_CALIBRATION_MONITOR.lock() {
        *guard = Some(monitor_info.clone());
    }

    // Create overlay covering only the target monitor
    let overlay = WebviewWindowBuilder::new(
        &app,
        "calibration-overlay",
        WebviewUrl::App("index.html".into()),
    )
    .title("Calibration")
    .inner_size(monitor_width as f64, monitor_height as f64)
    .position(monitor_x as f64, monitor_y as f64)
    .always_on_top(true)
    .decorations(false)
    .transparent(true)
    .skip_taskbar(true)
    .resizable(false)
    .closable(false)
    .minimizable(false)
    .maximizable(false)
    .visible(false)
    .focused(true)
    .accept_first_mouse(true)
    .build()
    .map_err(|e| format!("Failed to create calibration overlay: {}", e))?;

    // Wait for content to load before showing
    thread::sleep(Duration::from_millis(100));

    // Show the window and ensure it has focus
    overlay.show().ok();
    overlay.set_focus().ok();
    overlay.set_always_on_top(true).ok();

    // Return monitor info so frontend can save it
    Ok(monitor_info)
}

#[tauri::command]
pub async fn close_calibration(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("calibration-overlay") {
        window
            .destroy()
            .map_err(|e| format!("Failed to close calibration overlay: {}", e))?;
    }

    // Clear the stored monitor info
    if let Ok(mut guard) = CURRENT_CALIBRATION_MONITOR.lock() {
        *guard = None;
    }

    // Emit event to main window
    if let Some(main_window) = app.get_webview_window("main") {
        main_window.emit("calibration-closed", ()).ok();
    }

    Ok(())
}

#[tauri::command]
pub async fn get_calibration_monitor() -> Result<Option<MonitorInfo>, String> {
    let monitor = CURRENT_CALIBRATION_MONITOR
        .lock()
        .map_err(|e| format!("Failed to lock monitor state: {}", e))?
        .clone();

    Ok(monitor)
}

#[tauri::command]
pub async fn save_calibration_regions(
    app: AppHandle,
    regions: Vec<CalibrationRegion>,
    window_width: u32,
    window_height: u32,
    monitor: Option<MonitorInfo>,
) -> Result<(), String> {
    let calibration_data = CalibrationData {
        regions,
        window_width,
        window_height,
        monitor,
    };

    let file_path = get_calibration_file_path(&app)?;
    let json = serde_json::to_string_pretty(&calibration_data)
        .map_err(|e| format!("Failed to serialize calibration data: {}", e))?;

    fs::write(&file_path, json)
        .map_err(|e| format!("Failed to write calibration file: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn load_calibration_regions(app: AppHandle) -> Result<CalibrationData, String> {
    let file_path = get_calibration_file_path(&app)?;

    if !file_path.exists() {
        return Ok(CalibrationData::default());
    }

    let json = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read calibration file: {}", e))?;

    let calibration_data: CalibrationData = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse calibration data: {}", e))?;

    Ok(calibration_data)
}

#[tauri::command]
pub async fn test_capture(app: AppHandle) -> Result<String, String> {
    use image::GenericImageView;

    // Load calibration data
    let calibration_data = load_calibration_regions(app).await?;

    if calibration_data.regions.is_empty() {
        return Err("No calibration regions found. Please calibrate first.".to_string());
    }

    let region = &calibration_data.regions[0];

    // Get all monitors
    let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;

    // Find the correct monitor based on saved calibration data
    let target_monitor = if let Some(ref saved_monitor) = calibration_data.monitor {
        // Find the monitor that matches the saved position
        monitors
            .iter()
            .find(|m| m.x() == saved_monitor.x && m.y() == saved_monitor.y)
            .or_else(|| monitors.iter().find(|m| m.is_primary()))
            .ok_or("No matching monitor found")?
    } else {
        monitors
            .iter()
            .find(|m| m.is_primary())
            .ok_or("No primary monitor found")?
    };

    let full_screenshot = target_monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    // Crop to the calibrated region (coordinates are relative to the monitor)
    let x = region.x as u32;
    let y = region.y as u32;
    let width = region.width as u32;
    let height = region.height as u32;

    // Validate bounds
    if x + width > full_screenshot.width() || y + height > full_screenshot.height() {
        return Err(format!(
            "Region ({},{} {}x{}) exceeds screen bounds ({}x{})",
            x,
            y,
            width,
            height,
            full_screenshot.width(),
            full_screenshot.height()
        ));
    }

    let cropped = full_screenshot.view(x, y, width, height).to_image();

    // Save to Desktop
    let home_dir = std::env::var("HOME").map_err(|_| "Could not get HOME directory")?;
    let output_path = format!("{}/Desktop/test_capture.png", home_dir);

    cropped
        .save(&output_path)
        .map_err(|e| format!("Failed to save image: {}", e))?;

    Ok(output_path)
}
