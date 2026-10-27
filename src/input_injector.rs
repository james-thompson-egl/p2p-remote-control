//! Input injection using Win32 SendInput API
//! Works in user mode - no admin required

use tracing::{info, debug, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseLeftDown,
    MouseLeftUp,
    MouseRightDown,
    MouseRightUp,
    MouseWheel { delta: i32 },
    KeyDown { keycode: u16 },
    KeyUp { keycode: u16 },
    Char { c: char },
}

pub struct InputInjector;

impl InputInjector {
    pub fn new() -> Self {
        info!("InputInjector initialized");
        Self
    }

    /// Inject a single input event
    pub fn inject(&self, event: &InputEvent) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Injecting input event: {:?}", event);

        #[cfg(windows)]
        {
            use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, MOUSEINPUT, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_MOVE, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, KEYBD_EVENT_FLAGS};

            unsafe {
                match event {
                    InputEvent::MouseMove { x, y } => {
                        let input = INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: *x,
                                    dy: *y,
                                    mouseData: 0,
                                    dwFlags: MOUSEEVENTF_MOVE,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::MouseLeftDown => {
                        let input = INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: 0,
                                    dy: 0,
                                    mouseData: 0,
                                    dwFlags: MOUSEEVENTF_LEFTDOWN,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::MouseLeftUp => {
                        let input = INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: 0,
                                    dy: 0,
                                    mouseData: 0,
                                    dwFlags: MOUSEEVENTF_LEFTUP,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::MouseRightDown => {
                        let input = INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: 0,
                                    dy: 0,
                                    mouseData: 0,
                                    dwFlags: MOUSEEVENTF_RIGHTDOWN,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::MouseRightUp => {
                        let input = INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: 0,
                                    dy: 0,
                                    mouseData: 0,
                                    dwFlags: MOUSEEVENTF_RIGHTUP,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::MouseWheel { delta } => {
                        let input = INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: 0,
                                    dy: 0,
                                    mouseData: *delta as u32,
                                    dwFlags: MOUSEEVENTF_WHEEL,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::KeyDown { keycode } => {
                        let input = INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VIRTUAL_KEY(*keycode),
                                    wScan: 0,
                                    dwFlags: KEYBD_EVENT_FLAGS(0),
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::KeyUp { keycode } => {
                        let input = INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: VIRTUAL_KEY(*keycode),
                                    wScan: 0,
                                    dwFlags: KEYEVENTF_KEYUP,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        };
                        Self::send_input(&[input])?;
                    }

                    InputEvent::Char { c } => {
                        // For character input, we need to handle virtual key codes
                        // This is a simplified version - full implementation would
                        // handle shift modifiers for uppercase/symbols
                        let vk = Self::char_to_vk(*c);
                        if vk != 0 {
                            // Key down
                            let input_down = INPUT {
                                r#type: INPUT_KEYBOARD,
                                Anonymous: INPUT_0 {
                                    ki: KEYBDINPUT {
                                        wVk: VIRTUAL_KEY(vk),
                                        wScan: 0,
                                        dwFlags: KEYBD_EVENT_FLAGS(0),
                                        time: 0,
                                        dwExtraInfo: 0,
                                    },
                                },
                            };
                            // Key up
                            let input_up = INPUT {
                                r#type: INPUT_KEYBOARD,
                                Anonymous: INPUT_0 {
                                    ki: KEYBDINPUT {
                                        wVk: VIRTUAL_KEY(vk),
                                        wScan: 0,
                                        dwFlags: KEYEVENTF_KEYUP,
                                        time: 0,
                                        dwExtraInfo: 0,
                                    },
                                },
                            };
                            Self::send_input(&[input_down, input_up])?;
                        }
                    }
                }
            }
        }

        #[cfg(not(windows))]
        {
            warn!("Input injection only supported on Windows");
        }

        Ok(())
    }

    #[cfg(windows)]
    unsafe fn send_input(inputs: &[windows::Win32::UI::Input::KeyboardAndMouse::INPUT]) -> Result<(), Box<dyn std::error::Error>> {
        use windows::Win32::UI::Input::KeyboardAndMouse::SendInput;

        let result = SendInput(inputs, std::mem::size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>() as i32);

        if result as usize == inputs.len() {
            Ok(())
        } else {
            Err(format!("SendInput failed: {} of {} inputs sent", result, inputs.len()).into())
        }
    }

    /// Convert a character to a virtual key code (simplified)
    fn char_to_vk(c: char) -> u16 {
        match c {
            'a'..='z' => (c as u8 - b'a' + 0x41) as u16, // VK_A = 0x41
            'A'..='Z' => (c as u8 - b'A' + 0x41) as u16,
            '0'..='9' => (c as u8 - b'0' + 0x30) as u16, // VK_0 = 0x30
            ' ' => 0x20, // VK_SPACE
            '\n' | '\r' => 0x0D, // VK_RETURN
            '\t' => 0x09, // VK_TAB
            '!' => 0x31, // Shift+1 (simplified)
            '@' => 0x32, // Shift+2
            _ => 0, // Unknown
        }
    }
}

impl Default for InputInjector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vk_constants() {
        use windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY;
        assert_eq!(VIRTUAL_KEY(vk::VK_RETURN).0, 0x0D);
        assert_eq!(VIRTUAL_KEY(vk::VK_ESCAPE).0, 0x1B);
        assert_eq!(VIRTUAL_KEY(vk::VK_SPACE).0, 0x20);
        assert_eq!(VIRTUAL_KEY(vk::VK_TAB).0, 0x09);
    }

    #[test]
    fn test_char_to_vk_lowercase() {
        assert_eq!(InputInjector::char_to_vk('a'), 0x41);
        assert_eq!(InputInjector::char_to_vk('z'), 0x5A);
    }

    #[test]
    fn test_char_to_vk_uppercase() {
        assert_eq!(InputInjector::char_to_vk('A'), 0x41);
        assert_eq!(InputInjector::char_to_vk('Z'), 0x5A);
    }

    #[test]
    fn test_char_to_vk_numbers() {
        assert_eq!(InputInjector::char_to_vk('0'), 0x30);
        assert_eq!(InputInjector::char_to_vk('9'), 0x39);
    }

    #[test]
    fn test_char_to_vk_special() {
        assert_eq!(InputInjector::char_to_vk(' '), 0x20);
        assert_eq!(InputInjector::char_to_vk('\n'), 0x0D);
        assert_eq!(InputInjector::char_to_vk('\t'), 0x09);
    }

    #[test]
    fn test_input_event_serialization() {
        let event = InputEvent::MouseMove { x: 100, y: 200 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("MouseMove"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_input_event_deserialization() {
        let json = r#"{"MouseMove":{"x":50,"y":100}}"#;
        let event: InputEvent = serde_json::from_str(json).unwrap();

        match event {
            InputEvent::MouseMove { x, y } => {
                assert_eq!(x, 50);
                assert_eq!(y, 100);
            }
            _ => panic!("Wrong event type"),
        }
    }
}

/// Common virtual key codes
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
