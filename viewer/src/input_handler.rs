//! Input handler for the viewer
//!
//! Captures local input and formats for remote transmission

use serde::{Deserialize, Serialize};

/// Remote input event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInput {
    pub r#type: &'static str,
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
}

impl RemoteInput {
    pub fn mouse_move(x: i32, y: i32) -> Self {
        Self {
            r#type: "mousemove",
            x: Some(x),
            y: Some(y),
            button: None,
            delta: None,
            code: None,
        }
    }

    pub fn mouse_down(button: u8) -> Self {
        Self {
            r#type: "mousedown",
            x: None,
            y: None,
            button: Some(button),
            delta: None,
            code: None,
        }
    }

    pub fn mouse_up(button: u8) -> Self {
        Self {
            r#type: "mouseup",
            x: None,
            y: None,
            button: Some(button),
            delta: None,
            code: None,
        }
    }

    pub fn mouse_wheel(delta: i32) -> Self {
        Self {
            r#type: "wheel",
            x: None,
            y: None,
            button: None,
            delta: Some(delta),
            code: None,
        }
    }

    pub fn key_down(code: u16) -> Self {
        Self {
            r#type: "keydown",
            x: None,
            y: None,
            button: None,
            delta: None,
            code: Some(code),
        }
    }

    pub fn key_up(code: u16) -> Self {
        Self {
            r#type: "keyup",
            x: None,
            y: None,
            button: None,
            delta: None,
            code: Some(code),
        }
    }
}
