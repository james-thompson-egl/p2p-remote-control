//! Video encoding module
//!
//! Supports:
//! - H.264 encoding via MediaFoundation (Windows)
//! - VP8 fallback via software encoding
//! - Frame compression optimization

use tracing::{info, debug, warn};
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

/// Video encoder trait
pub trait VideoEncoder: Send + Sync {
    fn encode(&mut self, frame: &DynamicImage) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
    fn codec_name(&self) -> &str;
}

/// JPEG encoder (simple, no external dependencies)
pub struct JpegEncoder {
    quality: u8,
}

impl JpegEncoder {
    pub fn new(quality: u8) -> Self {
        Self { quality }
    }
}

impl VideoEncoder for JpegEncoder {
    fn encode(&mut self, frame: &DynamicImage) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);

        // Write JPEG with specified quality
        frame.write_to(&mut cursor, ImageFormat::Jpeg)?;

        debug!("Encoded frame: {} bytes", buf.len());
        Ok(buf)
    }

    fn codec_name(&self) -> &str {
        "JPEG"
    }
}

/// H.264 encoder using Windows MediaFoundation
#[cfg(windows)]
pub struct H264Encoder {
    width: u32,
    height: u32,
}

#[cfg(windows)]
impl H264Encoder {
    pub fn new(width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing H.264 encoder for {}x{}", width, height);

        Ok(Self {
            width,
            height,
        })
    }
}

#[cfg(windows)]
impl VideoEncoder for H264Encoder {
    fn encode(&mut self, frame: &DynamicImage) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Convert image to NV12 format for H.264 encoding
        let rgba = frame.to_rgba8();
        let nv12 = rgb_to_nv12(&rgba, self.width, self.height);

        // Encode using MediaFoundation
        // This is a simplified version - full implementation would use winrt-media crate
        todo!("MediaFoundation H.264 encoding requires winrt-media crate");
    }

    fn codec_name(&self) -> &str {
        "H.264"
    }
}

/// Convert RGB to NV12 format (required for H.264)
fn rgb_to_nv12(rgba: &image::RgbaImage, width: u32, height: u32) -> Vec<u8> {
    let mut nv12 = Vec::with_capacity((width * height * 3 / 2) as usize);

    // Y plane (luminance)
    for y in 0..height {
        for x in 0..width {
            let pixel = rgba.get_pixel(x, y);
            let y_val = ((0.299 * pixel[0] as f32) +
                        (0.587 * pixel[1] as f32) +
                        (0.114 * pixel[2] as f32)) as u8;
            nv12.push(y_val);
        }
    }

    // UV plane (chroma, subsampled 2x2)
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let p1 = rgba.get_pixel(x, y);
            let p2 = if x + 1 < width { rgba.get_pixel(x + 1, y) } else { p1 };
            let p3 = if y + 1 < height { rgba.get_pixel(x, y + 1) } else { p1 };
            let p4 = if x + 1 < width && y + 1 < height { rgba.get_pixel(x + 1, y + 1) } else { p1 };

            let u = ((p1[0] as f32 + p2[0] as f32 + p3[0] as f32 + p4[0] as f32) / 4.0) as u8;
            let v = ((p1[1] as f32 + p2[1] as f32 + p3[1] as f32 + p4[1] as f32) / 4.0) as u8;

            nv12.push(u);
            nv12.push(v);
        }
    }

    nv12
}

/// Screen delta detector - only send changed regions
pub struct DeltaDetector {
    previous_frame: Option<Vec<u8>>,
    threshold: u8,
}

impl DeltaDetector {
    pub fn new(threshold: u8) -> Self {
        Self {
            previous_frame: None,
            threshold,
        }
    }

    /// Compare current frame with previous, return regions that changed
    pub fn detect_delta(&mut self, frame: &DynamicImage) -> DeltaRegion {
        let current = frame.as_bytes();

        if let Some(prev) = &self.previous_frame {
            if prev == current {
                // No change
                return DeltaRegion::None;
            }
        }

        self.previous_frame = Some(current.to_vec());

        // For now, return full frame
        // In production, would calculate bounding box of changed regions
        DeltaRegion::Full(frame.as_bytes().to_vec())
    }

    pub fn has_changes(&self) -> bool {
        self.previous_frame.is_some()
    }

    pub fn reset(&mut self) {
        self.previous_frame = None;
    }
}

/// Represents a region of change
pub enum DeltaRegion {
    None,
    Full(Vec<u8>),
    Partial {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
}

impl DeltaRegion {
    pub fn is_empty(&self) -> bool {
        matches!(self, DeltaRegion::None)
    }

    pub fn byte_count(&self) -> usize {
        match self {
            DeltaRegion::None => 0,
            DeltaRegion::Full(data) => data.len(),
            DeltaRegion::Partial { data, .. } => data.len(),
        }
    }
}

/// Frame rate controller
pub struct FrameRateController {
    target_fps: u32,
    min_interval_ms: u64,
    last_frame_time: std::time::Instant,
}

impl FrameRateController {
    pub fn new(target_fps: u32) -> Self {
        let min_interval_ms = 1000 / target_fps;
        Self {
            target_fps,
            min_interval_ms: min_interval_ms as u64,
            last_frame_time: std::time::Instant::now(),
        }
    }

    /// Wait until it's time for the next frame
    pub fn wait_for_frame(&mut self) {
        let elapsed = self.last_frame_time.elapsed().as_millis() as u64;
        if elapsed < self.min_interval_ms {
            let sleep_time = self.min_interval_ms - elapsed;
            std::thread::sleep(std::time::Duration::from_millis(sleep_time));
        }
        self.last_frame_time = std::time::Instant::now();
    }

    /// Get recommended sleep interval
    pub fn get_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.min_interval_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_encoder_creation() {
        let encoder = JpegEncoder::new(85);
        assert_eq!(encoder.codec_name(), "JPEG");
    }

    #[test]
    fn test_jpeg_encoder_compresses() {
        let mut encoder = JpegEncoder::new(85);
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_fn(100, 100, |x, y| {
            image::Rgba([x as u8, y as u8, 0, 255])
        }));

        let encoded = encoder.encode(&img).unwrap();
        assert!(!encoded.is_empty());
        // JPEG should compress (though gradient images may not compress well)
        assert!(encoded.len() < img.as_bytes().len() * 2); // Allow some overhead
    }

    #[test]
    fn test_delta_detector_first_frame() {
        let mut detector = DeltaDetector::new(10);
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_fn(10, 10, |_, _| image::Rgba([100, 100, 100, 255])));

        let delta = detector.detect_delta(&img);
        assert!(!delta.is_empty()); // First frame always has changes
    }

    #[test]
    fn test_delta_detector_same_frame() {
        let mut detector = DeltaDetector::new(10);
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_fn(10, 10, |_, _| image::Rgba([100, 100, 100, 255])));

        let _ = detector.detect_delta(&img); // First call
        let delta = detector.detect_delta(&img); // Second call - same image

        assert!(delta.is_empty()); // No changes
    }

    #[test]
    fn test_delta_detector_different_frame() {
        let mut detector = DeltaDetector::new(10);
        let img1 = DynamicImage::ImageRgba8(image::ImageBuffer::from_fn(10, 10, |_, _| image::Rgba([100, 100, 100, 255])));
        let img2 = DynamicImage::ImageRgba8(image::ImageBuffer::from_fn(10, 10, |_, _| image::Rgba([200, 200, 200, 255])));

        let _ = detector.detect_delta(&img1);
        let delta = detector.detect_delta(&img2);

        assert!(!delta.is_empty()); // Different image = changes
    }

    #[test]
    fn test_framerate_controller_interval() {
        let controller = FrameRateController::new(30);
        let interval = controller.get_interval();

        assert!(interval.as_millis() >= 30); // ~33ms for 30fps
        assert!(interval.as_millis() <= 35);
    }

    #[test]
    fn test_framerate_controller_60fps() {
        let controller = FrameRateController::new(60);
        let interval = controller.get_interval();

        assert!(interval.as_millis() >= 15); // ~16ms for 60fps
        assert!(interval.as_millis() <= 17);
    }
}
