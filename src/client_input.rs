//! Client-side input capture
//!
//! Captures local mouse and keyboard events on the viewer machine
//! Sends them to the remote host via WebRTC data channel

use tracing::{info, debug, warn};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::input_injector::InputEvent;

/// Input event sent over the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInput {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ch: Option<String>,
}

impl RemoteInput {
    pub fn mouse_move(x: i32, y: i32) -> Self {
        Self {
            r#type: "mousemove".to_string(),
            x: Some(x),
            y: Some(y),
            button: None,
            delta: None,
            code: None,
            ch: None,
        }
    }

    pub fn mouse_down(button: u8) -> Self {
        Self {
            r#type: "mousedown".to_string(),
            x: None,
            y: None,
            button: Some(button),
            delta: None,
            code: None,
            ch: None,
        }
    }

    pub fn mouse_up(button: u8) -> Self {
        Self {
            r#type: "mouseup".to_string(),
            x: None,
            y: None,
            button: Some(button),
            delta: None,
            code: None,
            ch: None,
        }
    }

    pub fn mouse_wheel(delta: i32) -> Self {
        Self {
            r#type: "wheel".to_string(),
            x: None,
            y: None,
            button: None,
            delta: Some(delta),
            code: None,
            ch: None,
        }
    }

    pub fn key_down(code: u16) -> Self {
        Self {
            r#type: "keydown".to_string(),
            x: None,
            y: None,
            button: None,
            delta: None,
            code: Some(code),
            ch: None,
        }
    }

    pub fn key_up(code: u16) -> Self {
        Self {
            r#type: "keyup".to_string(),
            x: None,
            y: None,
            button: None,
            delta: None,
            code: Some(code),
            ch: None,
        }
    }

    pub fn char(ch: char) -> Self {
        Self {
            r#type: "char".to_string(),
            x: None,
            y: None,
            button: None,
            delta: None,
            code: None,
            ch: Some(ch.to_string()),
        }
    }
}

/// Windows input hook for capturing mouse and keyboard
#[cfg(windows)]
pub struct InputHook {
    running: Arc<AtomicBool>,
    mouse_callback: Option<Box<dyn Fn(InputEvent) + Send + Sync>>,
    key_callback: Option<Box<dyn Fn(InputEvent) + Send + Sync>>,
}

#[cfg(windows)]
impl InputHook {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            mouse_callback: None,
            key_callback: None,
        }
    }

    /// Start the input hook
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting input hook");
        self.running.store(true, Ordering::SeqCst);

        // Note: Full global hooks require a DLL injection which is complex.
        // For now, we use a simpler approach: focus-based capture.
        // The viewer window must be focused to capture input.

        Ok(())
    }

    /// Stop the input hook
    pub fn stop(&mut self) {
        info!("Stopping input hook");
        self.running.store(false, Ordering::SeqCst);
    }

    /// Set mouse event callback
    pub fn on_mouse<F>(&mut self, callback: F)
    where
        F: Fn(InputEvent) + Send + Sync + 'static,
    {
        self.mouse_callback = Some(Box::new(callback));
    }

    /// Set keyboard event callback
    pub fn on_key<F>(&mut self, callback: F)
    where
        F: Fn(InputEvent) + Send + Sync + 'static,
    {
        self.key_callback = Some(Box::new(callback));
    }
}

/// Simple input capture using raw input (no hooks required)
pub struct SimpleInputCapture {
    running: Arc<AtomicBool>,
}

impl SimpleInputCapture {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start capturing input in a background task
    pub fn start<F>(&self, on_input: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn(InputEvent) + Send + Sync + 'static,
    {
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            info!("Input capture started");

            // In a full implementation, this would:
            // 1. Register for raw input from mouse/keyboard
            // 2. Process WM_INPUT messages in a window procedure
            // 3. Convert to InputEvent and call on_input

            // For now, we provide a placeholder that would be
            // replaced with actual input capture in a GUI application

            while running.load(Ordering::SeqCst) {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }

            info!("Input capture stopped");
        })
    }

    /// Stop capturing input
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Default for SimpleInputCapture {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert raw input to InputEvent
#[cfg(windows)]
pub fn convert_raw_input(msg: u32, wparam: usize, lparam: isize) -> Option<InputEvent> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::{WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_MOUSEWHEEL, WM_KEYDOWN, WM_KEYUP};

    match msg {
        WM_MOUSEMOVE => {
            // Extract coordinates from lparam
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            Some(InputEvent::MouseMove { x, y })
        }
        WM_LBUTTONDOWN => Some(InputEvent::MouseLeftDown),
        WM_LBUTTONUP => Some(InputEvent::MouseLeftUp),
        WM_RBUTTONDOWN => Some(InputEvent::MouseRightDown),
        WM_RBUTTONUP => Some(InputEvent::MouseRightUp),
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as i16 as i32;
            Some(InputEvent::MouseWheel { delta })
        }
        WM_KEYDOWN => {
            let keycode = (wparam & 0xFFFF) as u16;
            Some(InputEvent::KeyDown { keycode })
        }
        WM_KEYUP => {
            let keycode = (wparam & 0xFFFF) as u16;
            Some(InputEvent::KeyUp { keycode })
        }
        _ => None,
    }
}

/// Virtual key codes
pub mod vk {
    pub const VK_LBUTTON: u16 = 0x01;
    pub const VK_RBUTTON: u16 = 0x02;
    pub const VK_CANCEL: u16 = 0x03;
    pub const VK_MBUTTON: u16 = 0x04;
    pub const VK_BACK: u16 = 0x08;
    pub const VK_TAB: u16 = 0x09;
    pub const VK_RETURN: u16 = 0x0D;
    pub const VK_SHIFT: u16 = 0x10;
    pub const VK_CONTROL: u16 = 0x11;
    pub const VK_MENU: u16 = 0x12; // Alt
    pub const VK_ESCAPE: u16 = 0x1B;
    pub const VK_SPACE: u16 = 0x20;
    pub const VK_LEFT: u16 = 0x25;
    pub const VK_UP: u16 = 0x26;
    pub const VK_RIGHT: u16 = 0x27;
    pub const VK_DOWN: u16 = 0x28;
    pub const VK_F1: u16 = 0x70;
    pub const VK_F12: u16 = 0x7B;
    pub const VK_SNAPSHOT: u16 = 0x2C; // Print Screen
    pub const VK_INSERT: u16 = 0x2D;
    pub const VK_DELETE: u16 = 0x2E;
    pub const VK_LWIN: u16 = 0x5B; // Windows key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_input_mouse_move() {
        let input = RemoteInput::mouse_move(100, 200);
        assert_eq!(input.r#type, "mousemove");
        assert_eq!(input.x, Some(100));
        assert_eq!(input.y, Some(200));
    }

    #[test]
    fn test_remote_input_mouse_down() {
        let input = RemoteInput::mouse_down(0);
        assert_eq!(input.r#type, "mousedown");
        assert_eq!(input.button, Some(0));
    }

    #[test]
    fn test_remote_input_mouse_up() {
        let input = RemoteInput::mouse_up(1);
        assert_eq!(input.r#type, "mouseup");
        assert_eq!(input.button, Some(1));
    }

    #[test]
    fn test_remote_input_mouse_wheel() {
        let input = RemoteInput::mouse_wheel(120);
        assert_eq!(input.r#type, "wheel");
        assert_eq!(input.delta, Some(120));
    }

    #[test]
    fn test_remote_input_key_down() {
        let input = RemoteInput::key_down(0x41);
        assert_eq!(input.r#type, "keydown");
        assert_eq!(input.code, Some(0x41));
    }

    #[test]
    fn test_remote_input_key_up() {
        let input = RemoteInput::key_up(0x41);
        assert_eq!(input.r#type, "keyup");
        assert_eq!(input.code, Some(0x41));
    }

    #[test]
    fn test_remote_input_serialization() {
        let input = RemoteInput::mouse_move(100, 200);
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("mousemove"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_remote_input_deserialization() {
        let json = r#"{"type":"mousedown","button":0}"#;
        let input: RemoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.r#type, "mousedown");
        assert_eq!(input.button, Some(0));
    }

    #[test]
    fn test_remote_input_roundtrip() {
        let original = RemoteInput::mouse_move(500, 600);
        let json = serde_json::to_string(&original).unwrap();
        let restored: RemoteInput = serde_json::from_str(&json).unwrap();

        assert_eq!(original.r#type, restored.r#type);
        assert_eq!(original.x, restored.x);
        assert_eq!(original.y, restored.y);
    }
}
