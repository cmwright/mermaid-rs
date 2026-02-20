use crate::ast::common::Color;
use crate::diagram::RenderConfig;
use crate::error::{MermaidError, Result};

/// Render an SVG string to PNG bytes.
///
/// This uses resvg to parse the SVG and render it to a raster image,
/// then encodes it as PNG using the png crate.
///
/// Default scale is 2.0 for retina-ready output (can be made configurable).
const DEFAULT_SCALE: f32 = 2.0;

pub fn render_png(svg: &str, config: &RenderConfig) -> Result<Vec<u8>> {
    // Parse SVG using resvg's usvg
    let mut opt = resvg::usvg::Options::default();

    // Set up font database with custom fonts from config
    let mut fontdb = resvg::usvg::fontdb::Database::new();

    // Load the custom font data if available
    if config.font_provider.font_ref().is_ok() {
        // Get font data and load it into fontdb
        let font_data = config.font_provider.font_data();
        fontdb.load_font_source(resvg::usvg::fontdb::Source::Binary(std::sync::Arc::new(
            font_data,
        )));
    }

    // Also load system fonts as fallback (not available on WASM)
    #[cfg(not(target_arch = "wasm32"))]
    fontdb.load_system_fonts();

    opt.fontdb = std::sync::Arc::new(fontdb);

    let doc = resvg::usvg::Tree::from_str(svg, &opt)
        .map_err(|e| MermaidError::Render(format!("Failed to parse SVG for PNG: {}", e)))?;

    // Get image dimensions and apply scale
    let size = doc.size();
    let width = (size.width() * DEFAULT_SCALE) as u32;
    let height = (size.height() * DEFAULT_SCALE) as u32;

    // Create pixmap
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| MermaidError::Render("Failed to create pixmap".to_string()))?;

    // Fill background if specified
    if let Some(bg) = &config.background {
        pixmap.fill(color_to_pixmap_color(bg));
    } else {
        // Default to theme background color
        let bg_color = &config.theme.background;
        pixmap.fill(color_to_pixmap_color(bg_color));
    }

    // Render SVG to pixmap using resvg
    resvg::render(
        &doc,
        resvg::tiny_skia::Transform::from_scale(DEFAULT_SCALE, DEFAULT_SCALE),
        &mut pixmap.as_mut(),
    );

    // Encode to PNG
    encode_png(&pixmap)
}

fn color_to_pixmap_color(color: &Color) -> resvg::tiny_skia::Color {
    // Convert our Color type to tiny-skia Color
    let (r, g, b, a) = match color {
        Color::Hex(hex) => hex_to_rgba(hex),
        Color::Named(name) => named_color_to_rgba(name),
        Color::None => (0, 0, 0, 0),
    };

    resvg::tiny_skia::Color::from_rgba8(r, g, b, a)
}

fn hex_to_rgba(hex: &str) -> (u8, u8, u8, u8) {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        3 => {
            // Short form: RGB
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0);
            (r, g, b, 255)
        }
        6 => {
            // Long form: RRGGBB
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            (r, g, b, 255)
        }
        8 => {
            // With alpha: RRGGBBAA
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            (r, g, b, a)
        }
        _ => (0, 0, 0, 255),
    }
}

fn named_color_to_rgba(name: &str) -> (u8, u8, u8, u8) {
    match name.to_lowercase().as_str() {
        "white" => (255, 255, 255, 255),
        "black" => (0, 0, 0, 255),
        "red" => (255, 0, 0, 255),
        "green" => (0, 128, 0, 255),
        "blue" => (0, 0, 255, 255),
        "yellow" => (255, 255, 0, 255),
        "cyan" => (0, 255, 255, 255),
        "magenta" => (255, 0, 255, 255),
        "transparent" => (0, 0, 0, 0),
        _ => (0, 0, 0, 255),
    }
}

fn encode_png(pixmap: &resvg::tiny_skia::Pixmap) -> Result<Vec<u8>> {
    let width = pixmap.width();
    let height = pixmap.height();
    let data = pixmap.data();

    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder
            .write_header()
            .map_err(|e| MermaidError::Render(format!("PNG header error: {}", e)))?;

        writer
            .write_image_data(data)
            .map_err(|e| MermaidError::Render(format!("PNG encode error: {}", e)))?;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::RenderConfig;
    use crate::font::FontProvider;

    fn test_svg() -> String {
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\" width=\"100\" height=\"100\">\n            <rect x=\"10\" y=\"10\" width=\"80\" height=\"80\" fill=\"#ff0000\"/>\n        </svg>".to_string()
    }

    #[test]
    fn test_render_png_basic() {
        let config = RenderConfig::default();
        let svg = test_svg();
        let png = render_png(&svg, &config);
        assert!(png.is_ok());

        let png_bytes = png.unwrap();
        // Check PNG magic bytes
        assert_eq!(
            &png_bytes[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        // Check file is not empty
        assert!(png_bytes.len() > 100);
    }

    #[test]
    fn test_hex_to_rgba_short() {
        assert_eq!(hex_to_rgba("#f00"), (255, 0, 0, 255));
        assert_eq!(hex_to_rgba("#0f0"), (0, 255, 0, 255));
        assert_eq!(hex_to_rgba("#00f"), (0, 0, 255, 255));
    }

    #[test]
    fn test_hex_to_rgba_long() {
        assert_eq!(hex_to_rgba("#ff0000"), (255, 0, 0, 255));
        assert_eq!(hex_to_rgba("#00ff00"), (0, 255, 0, 255));
        assert_eq!(hex_to_rgba("#0000ff"), (0, 0, 255, 255));
        assert_eq!(hex_to_rgba("#ffffff"), (255, 255, 255, 255));
        assert_eq!(hex_to_rgba("#000000"), (0, 0, 0, 255));
    }

    #[test]
    fn test_hex_to_rgba_with_alpha() {
        assert_eq!(hex_to_rgba("#ff000080"), (255, 0, 0, 128));
        assert_eq!(hex_to_rgba("#00000000"), (0, 0, 0, 0));
        assert_eq!(hex_to_rgba("#ffffffff"), (255, 255, 255, 255));
    }

    #[test]
    fn test_hex_to_rgba_invalid() {
        // Invalid hex should fallback to black
        assert_eq!(hex_to_rgba("#ggg"), (0, 0, 0, 255));
        assert_eq!(hex_to_rgba("#xyz"), (0, 0, 0, 255));
        assert_eq!(hex_to_rgba(""), (0, 0, 0, 255));
    }

    #[test]
    fn test_hex_to_rgba_invalid_length() {
        // Lengths not 3, 6, or 8 should hit the _ fallback
        assert_eq!(hex_to_rgba("#ab"), (0, 0, 0, 255));
        assert_eq!(hex_to_rgba("#abcde"), (0, 0, 0, 255));
        assert_eq!(hex_to_rgba("#abcdefaa0"), (0, 0, 0, 255));
    }

    #[test]
    fn test_color_to_pixmap_color_named() {
        let color = Color::Named("red".to_string());
        let pixmap_color = color_to_pixmap_color(&color);
        assert!(pixmap_color.red() > 0.9);
    }

    #[test]
    fn test_color_to_pixmap_color_none() {
        let color = Color::None;
        let pixmap_color = color_to_pixmap_color(&color);
        assert!(pixmap_color.alpha() < 0.01);
    }

    #[test]
    fn test_named_color_to_rgba() {
        assert_eq!(named_color_to_rgba("white"), (255, 255, 255, 255));
        assert_eq!(named_color_to_rgba("WHITE"), (255, 255, 255, 255));
        assert_eq!(named_color_to_rgba("black"), (0, 0, 0, 255));
        assert_eq!(named_color_to_rgba("red"), (255, 0, 0, 255));
        assert_eq!(named_color_to_rgba("green"), (0, 128, 0, 255));
        assert_eq!(named_color_to_rgba("blue"), (0, 0, 255, 255));
        assert_eq!(named_color_to_rgba("yellow"), (255, 255, 0, 255));
        assert_eq!(named_color_to_rgba("cyan"), (0, 255, 255, 255));
        assert_eq!(named_color_to_rgba("magenta"), (255, 0, 255, 255));
        assert_eq!(named_color_to_rgba("transparent"), (0, 0, 0, 0));
        assert_eq!(named_color_to_rgba("unknown"), (0, 0, 0, 255));
    }

    #[test]
    fn test_color_to_pixmap_color() {
        let color = Color::Hex("#ff0000".to_string());
        let pixmap_color = color_to_pixmap_color(&color);
        // Check that red component is set (resvg uses premultiplied alpha internally)
        assert!(pixmap_color.red() > 0.9);
        assert!(pixmap_color.green() < 0.1);
        assert!(pixmap_color.blue() < 0.1);
    }

    #[test]
    fn test_render_png_with_custom_background() {
        let mut config = RenderConfig::default();
        config.background = Some(Color::Hex("#0000ff".to_string()));

        let svg = test_svg();
        let png = render_png(&svg, &config);
        assert!(png.is_ok());
        assert!(png.unwrap().len() > 100);
    }

    #[test]
    fn test_render_png_with_transparent_background() {
        let mut config = RenderConfig::default();
        config.background = Some(Color::None);

        let svg = test_svg();
        let png = render_png(&svg, &config);
        assert!(png.is_ok());
        assert!(png.unwrap().len() > 100);
    }

    #[test]
    fn test_render_png_with_custom_font_provider() {
        // Exercise the font loading path: config.font_provider.font_ref().is_ok()
        // and fontdb.load_font_source with custom font data (FontData::Owned path)
        let default_provider = FontProvider::default_font();
        let font_bytes = default_provider.font_data();
        let custom_provider = FontProvider::from_bytes(font_bytes).expect("valid font bytes");
        let mut config = RenderConfig::default();
        config.font_provider = custom_provider;

        let svg = test_svg();
        let png = render_png(&svg, &config);
        assert!(png.is_ok());
        assert!(png.unwrap().len() > 100);
    }
}
