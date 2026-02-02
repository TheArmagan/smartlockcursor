//! SmartLockCursor - A Windows utility that locks the mouse cursor to fullscreen windows
//!
//! This utility detects when a window goes fullscreen and clips the mouse cursor
//! to the bounds of the display containing that window.

use std::mem::zeroed;
use std::ptr::null_mut;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ClipCursor, GetClassNameW, GetForegroundWindow, GetWindowRect,
};

/// Represents a monitor's information
#[derive(Debug, Clone)]
struct MonitorBounds {
    rect: RECT,
    #[allow(dead_code)]
    handle: HMONITOR,
}

/// Collects all monitor bounds in the system
fn get_all_monitors() -> Vec<MonitorBounds> {
    let mut monitors: Vec<MonitorBounds> = Vec::new();

    unsafe extern "system" fn enum_monitor_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(lparam.0 as *mut Vec<MonitorBounds>);

        let mut monitor_info: MONITORINFO = zeroed();
        monitor_info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;

        if GetMonitorInfoW(hmonitor, &mut monitor_info).as_bool() {
            monitors.push(MonitorBounds {
                rect: monitor_info.rcMonitor,
                handle: hmonitor,
            });
        }

        TRUE
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_monitor_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }

    monitors
}

/// Gets monitor rect for a specific monitor handle
fn get_monitor_rect(hmonitor: HMONITOR) -> Option<RECT> {
    unsafe {
        let mut monitor_info: MONITORINFO = zeroed();
        monitor_info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;

        if GetMonitorInfoW(hmonitor, &mut monitor_info).as_bool() {
            Some(monitor_info.rcMonitor)
        } else {
            None
        }
    }
}

/// Checks if a window is in fullscreen mode and returns the monitor rect if so
fn check_fullscreen(hwnd: HWND) -> Option<RECT> {
    if hwnd.0 == null_mut() {
        return None;
    }

    unsafe {
        // Get window rect
        let mut window_rect: RECT = zeroed();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return None;
        }

        // Get the monitor this window is primarily on
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let monitor_rect = get_monitor_rect(hmonitor)?;

        // Calculate dimensions
        let window_width = window_rect.right - window_rect.left;
        let window_height = window_rect.bottom - window_rect.top;
        let monitor_width = monitor_rect.right - monitor_rect.left;
        let monitor_height = monitor_rect.bottom - monitor_rect.top;

        // Allow small tolerance (some apps have slight differences)
        let tolerance = 5;

        // Check if window size matches monitor size (with tolerance)
        let width_match = (window_width - monitor_width).abs() <= tolerance;
        let height_match = (window_height - monitor_height).abs() <= tolerance;

        // Check if window position matches monitor position (with tolerance)
        let left_match = (window_rect.left - monitor_rect.left).abs() <= tolerance;
        let top_match = (window_rect.top - monitor_rect.top).abs() <= tolerance;

        if width_match && height_match && left_match && top_match {
            return Some(monitor_rect);
        }

        // Alternative: window completely covers or exceeds monitor bounds
        if window_rect.left <= monitor_rect.left + tolerance
            && window_rect.top <= monitor_rect.top + tolerance
            && window_rect.right >= monitor_rect.right - tolerance
            && window_rect.bottom >= monitor_rect.bottom - tolerance
            && window_width >= monitor_width - tolerance
            && window_height >= monitor_height - tolerance
        {
            return Some(monitor_rect);
        }

        None
    }
}

/// Clips the cursor to the specified rectangle
fn clip_cursor_to_rect(rect: &RECT) -> bool {
    unsafe { ClipCursor(Some(rect)).is_ok() }
}

/// Releases the cursor clip
fn release_cursor_clip() -> bool {
    unsafe { ClipCursor(None).is_ok() }
}

/// Compare two RECTs for equality
fn rects_equal(a: &RECT, b: &RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}

/// Check if the current foreground window is the Alt+Tab task switcher
fn is_task_switcher(hwnd: HWND) -> bool {
    if hwnd.0 == null_mut() {
        return false;
    }

    unsafe {
        let mut class_name = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len == 0 {
            return false;
        }

        let class_str = String::from_utf16_lossy(&class_name[..len as usize]);

        // Windows Alt+Tab switcher class names
        // "MultitaskingViewFrame" - Windows 10/11 Alt+Tab
        // "TaskSwitcherWnd" - Older Windows Alt+Tab
        // "XamlExplorerHostIslandWindow" - Windows 11 Alt+Tab variant
        // "Windows.UI.Core.CoreWindow" - Can be task view
        class_str.contains("MultitaskingView")
            || class_str.contains("TaskSwitcher")
            || class_str.contains("XamlExplorerHostIslandWindow")
            || class_str == "ForegroundStaging"
    }
}

/// Main application state
struct AppState {
    is_cursor_locked: bool,
    locked_to_hwnd: isize,
    current_monitor_rect: Option<RECT>,
    // Counter for grace period - prevents immediate unlock on transient focus changes
    stable_count: u32,
    // Track if we're in Alt+Tab mode
    alt_tab_active: bool,
    // Track if user switched away after Alt+Tab (don't re-lock until they click fullscreen window)
    user_switched_away: bool,
    // Remember the fullscreen window we were locked to
    remembered_fullscreen_hwnd: isize,
}

impl AppState {
    fn new() -> Self {
        Self {
            is_cursor_locked: false,
            locked_to_hwnd: 0,
            current_monitor_rect: None,
            stable_count: 0,
            alt_tab_active: false,
            user_switched_away: false,
            remembered_fullscreen_hwnd: 0,
        }
    }

    fn update(&mut self) {
        unsafe {
            let foreground = GetForegroundWindow();

            // Handle case when no foreground window
            if foreground.0 == null_mut() {
                if self.is_cursor_locked {
                    self.stable_count = self.stable_count.saturating_sub(1);
                    if self.stable_count == 0 {
                        release_cursor_clip();
                        self.is_cursor_locked = false;
                        self.locked_to_hwnd = 0;
                        self.current_monitor_rect = None;
                        println!("[INFO] No foreground window, cursor released");
                    } else {
                        // Keep re-applying clip during grace period
                        if let Some(ref rect) = self.current_monitor_rect {
                            let _ = clip_cursor_to_rect(rect);
                        }
                    }
                }
                return;
            }

            // Check if Alt+Tab task switcher is active
            if is_task_switcher(foreground) {
                if !self.alt_tab_active {
                    self.alt_tab_active = true;
                    // Remember which fullscreen window we were locked to
                    if self.is_cursor_locked {
                        self.remembered_fullscreen_hwnd = self.locked_to_hwnd;
                    }
                    // Temporarily release cursor for Alt+Tab navigation
                    release_cursor_clip();
                    println!("[INFO] Alt+Tab detected, cursor temporarily released");
                }
                // Don't do anything else while in Alt+Tab
                return;
            }

            // If we were in Alt+Tab and now we're not
            if self.alt_tab_active {
                self.alt_tab_active = false;
                let hwnd_value = foreground.0 as isize;

                // Check if user switched to a different window than the fullscreen one
                if self.remembered_fullscreen_hwnd != 0
                    && hwnd_value != self.remembered_fullscreen_hwnd
                {
                    // User switched to a different window after Alt+Tab
                    self.user_switched_away = true;
                    self.is_cursor_locked = false;
                    self.locked_to_hwnd = 0;
                    self.current_monitor_rect = None;
                    self.stable_count = 0;
                    println!(
                        "[INFO] Alt+Tab ended - switched to different window, cursor stays free"
                    );
                } else if self.remembered_fullscreen_hwnd != 0
                    && hwnd_value == self.remembered_fullscreen_hwnd
                {
                    // User returned to the same fullscreen window
                    self.user_switched_away = false;
                    println!("[INFO] Alt+Tab ended - returned to fullscreen window");
                } else {
                    println!("[INFO] Alt+Tab ended");
                }
                self.remembered_fullscreen_hwnd = 0;
            }

            let hwnd_value = foreground.0 as isize;

            // Check if current window is fullscreen
            if let Some(monitor_rect) = check_fullscreen(foreground) {
                // Window is fullscreen

                // If user switched away after Alt+Tab, only re-lock if they click the fullscreen window
                if self.user_switched_away {
                    // User clicked on a fullscreen window - clear the switched_away flag and lock
                    self.user_switched_away = false;
                    println!("[INFO] User clicked fullscreen window, re-enabling lock");
                }

                let is_new_lock = !self.is_cursor_locked;
                let is_different_window = self.locked_to_hwnd != hwnd_value;
                let is_different_monitor = self
                    .current_monitor_rect
                    .map_or(true, |r| !rects_equal(&r, &monitor_rect));

                if is_new_lock || is_different_window || is_different_monitor {
                    // New fullscreen detected
                    if clip_cursor_to_rect(&monitor_rect) {
                        self.is_cursor_locked = true;
                        self.locked_to_hwnd = hwnd_value;
                        self.current_monitor_rect = Some(monitor_rect);
                        self.stable_count = 50; // 5 second grace period (50 * 100ms)
                        println!(
                            "[INFO] Cursor locked to monitor: ({}, {}) - ({}, {})",
                            monitor_rect.left,
                            monitor_rect.top,
                            monitor_rect.right,
                            monitor_rect.bottom
                        );
                    }
                } else {
                    // Same fullscreen window - refresh the clip and reset grace period
                    self.stable_count = 50;
                    // Re-apply clip periodically (some apps/overlays can steal it)
                    if let Some(ref rect) = self.current_monitor_rect {
                        let _ = clip_cursor_to_rect(rect);
                    }
                }
            } else {
                // Window is NOT fullscreen

                // If user switched away, don't apply any lock logic
                if self.user_switched_away {
                    // User is on a non-fullscreen window after Alt+Tab, do nothing
                    return;
                }

                if self.is_cursor_locked {
                    self.stable_count = self.stable_count.saturating_sub(1);

                    if self.stable_count == 0 {
                        // Grace period expired, release cursor
                        release_cursor_clip();
                        self.is_cursor_locked = false;
                        self.locked_to_hwnd = 0;
                        self.current_monitor_rect = None;
                        println!("[INFO] Fullscreen exited, cursor released");
                    } else {
                        // Still in grace period - keep clip active
                        // This handles transient overlays, notifications, etc.
                        if let Some(ref rect) = self.current_monitor_rect {
                            let _ = clip_cursor_to_rect(rect);
                        }
                    }
                }
            }
        }
    }
}

fn print_banner() {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║              SmartLockCursor v0.1.0                       ║");
    println!("║  Automatically locks cursor to fullscreen windows         ║");
    println!("╠═══════════════════════════════════════════════════════════╣");
    println!("║  Press Ctrl+C to exit                                     ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
}

fn print_monitor_info() {
    let monitors = get_all_monitors();
    println!("[INFO] Detected {} monitor(s):", monitors.len());
    for (i, monitor) in monitors.iter().enumerate() {
        let width = monitor.rect.right - monitor.rect.left;
        let height = monitor.rect.bottom - monitor.rect.top;
        println!(
            "  Monitor {}: {}x{} at ({}, {})",
            i + 1,
            width,
            height,
            monitor.rect.left,
            monitor.rect.top
        );
    }
    println!();
}

fn main() {
    print_banner();
    print_monitor_info();

    println!("[INFO] Monitoring for fullscreen windows...");
    println!();

    let mut state = AppState::new();

    // Set up Ctrl+C handler to release cursor on exit
    ctrlc_handler();

    // Main loop - check every 100ms
    loop {
        state.update();
        thread::sleep(Duration::from_millis(100));
    }
}

/// Sets up a handler to release cursor clip on Ctrl+C
fn ctrlc_handler() {
    std::panic::set_hook(Box::new(|_| unsafe {
        let _ = ClipCursor(None);
    }));

    // Handle Ctrl+C
    let _ = ctrlc::set_handler(move || {
        println!("\n[INFO] Shutting down, releasing cursor...");
        unsafe {
            let _ = ClipCursor(None);
        }
        std::process::exit(0);
    });
}
