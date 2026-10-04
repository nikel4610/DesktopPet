use std::path::Path;

use image::{ImageReader, Limits, RgbaImage};

// A safety ceiling per RGBA frame, not a target for the application's memory use.
pub const MAX_FRAME_BYTES: u64 = 64 * 1024 * 1024;

pub fn frame_bytes(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err("image dimensions must be positive and fit Win32 coordinates".into());
    }
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("image byte count overflow")?;
    if bytes > MAX_FRAME_BYTES {
        return Err(format!(
            "RGBA frame requires {bytes} bytes; limit is {MAX_FRAME_BYTES}"
        ));
    }
    usize::try_from(bytes).map_err(|_| "image byte count does not fit this platform".into())
}

pub fn scaled_dimensions(width: u32, height: u32, scale: f32) -> Result<(u32, u32), String> {
    frame_bytes(width, height)?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err("scale must be finite and positive".into());
    }
    let scaled_width = (f64::from(width) * f64::from(scale)).round();
    let scaled_height = (f64::from(height) * f64::from(scale)).round();
    if scaled_width < 1.0
        || scaled_height < 1.0
        || scaled_width > f64::from(i32::MAX)
        || scaled_height > f64::from(i32::MAX)
    {
        return Err(format!(
            "scale {scale} produces invalid Win32 image dimensions"
        ));
    }
    let size = (scaled_width as u32, scaled_height as u32);
    frame_bytes(size.0, size.1)?;
    Ok(size)
}

pub fn decode_frame(path: &Path, scale: f32) -> Result<RgbaImage, String> {
    let dimensions = image::image_dimensions(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    scaled_dimensions(dimensions.0, dimensions.1, scale)?;
    let mut reader = ImageReader::open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_FRAME_BYTES);
    limits.max_image_width = Some(dimensions.0);
    limits.max_image_height = Some(dimensions.1);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| format!("failed to decode {}: {error}", path.display()))?
        .to_rgba8();
    scaled_dimensions(image.width(), image.height(), scale)?;
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_excessive_source_and_scaled_allocations() {
        assert!(frame_bytes(0, 1).is_err());
        assert!(frame_bytes(u32::MAX, u32::MAX).is_err());
        assert!(frame_bytes(50_000, 50_000).is_err());
        assert!(scaled_dimensions(96, 96, 1000.0).is_err());
        assert!(scaled_dimensions(96, 96, f32::INFINITY).is_err());
        assert!(scaled_dimensions(96, 96, 0.001).is_err());
        assert_eq!(scaled_dimensions(192, 208, 0.5).unwrap(), (96, 104));
        assert_eq!(frame_bytes(4096, 4096).unwrap() as u64, MAX_FRAME_BYTES);
    }
}
