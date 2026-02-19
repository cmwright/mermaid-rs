//! Integration tests for PNG rendering.

use mermaid_core::{render, OutputFormat, RenderConfig};

#[test]
fn test_png_rendering_flowchart() {
    let source = r#"flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[OK]
    B -->|No| D[Fail]
"#;

    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;

    let result = render(source, &config);
    assert!(result.is_ok(), "PNG rendering failed: {:?}", result.err());

    let output = result.unwrap();
    let png_bytes = output.into_png().expect("Expected PNG output");

    // Verify PNG magic bytes
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );

    // Verify file is reasonable size
    assert!(png_bytes.len() > 100, "PNG file too small");

    // Verify dimensions are reasonable (at least 100x100 since we have 4 nodes)
    // PNG dimensions are stored at offset 16-24 (width at 16, height at 20)
    let width = u32::from_be_bytes([png_bytes[16], png_bytes[17], png_bytes[18], png_bytes[19]]);
    let height = u32::from_be_bytes([png_bytes[20], png_bytes[21], png_bytes[22], png_bytes[23]]);
    assert!(width >= 100, "PNG width too small: {}", width);
    assert!(height >= 100, "PNG height too small: {}", height);
}

#[test]
fn test_png_rendering_pie_chart() {
    let source = r#"pie title Test Pie Chart
    "Slice 1" : 30
    "Slice 2" : 70
"#;

    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;

    let result = render(source, &config);
    assert!(result.is_ok(), "PNG rendering failed: {:?}", result.err());

    let output = result.unwrap();
    let png_bytes = output.into_png().expect("Expected PNG output");

    // Verify PNG magic bytes
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
    assert!(png_bytes.len() > 100);
}

#[test]
fn test_png_rendering_sequence() {
    let source = r#"sequenceDiagram
    Alice->>Bob: Hello
    Bob->>Alice: Hi!
"#;

    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;

    let result = render(source, &config);
    assert!(result.is_ok(), "PNG rendering failed: {:?}", result.err());

    let output = result.unwrap();
    let png_bytes = output.into_png().expect("Expected PNG output");

    // Verify PNG magic bytes
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
    assert!(png_bytes.len() > 100);
}

#[test]
fn test_png_rendering_gitgraph() {
    let source = r#"gitGraph:
    commit "Initial"
    branch develop
    checkout develop
    commit "Feature work"
    checkout main
    merge develop
"#;

    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;

    let result = render(source, &config);
    assert!(result.is_ok(), "PNG rendering failed: {:?}", result.err());

    let output = result.unwrap();
    let png_bytes = output.into_png().expect("Expected PNG output");

    // Verify PNG magic bytes
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
    assert!(png_bytes.len() > 100);
}

#[test]
fn test_png_with_transparent_background() {
    use mermaid_core::ast::common::Color;

    let source = r#"flowchart TD
    A[Node A] --> B[Node B]
"#;

    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;
    config.background = Some(Color::None);

    let result = render(source, &config);
    assert!(result.is_ok(), "PNG rendering failed: {:?}", result.err());

    let output = result.unwrap();
    let png_bytes = output.into_png().expect("Expected PNG output");

    // Verify it's a valid PNG
    assert_eq!(
        &png_bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
    assert!(png_bytes.len() > 100);
}

#[test]
fn test_render_output_as_svg() {
    let source = r#"flowchart TD
    A[Start]
"#;

    let config = RenderConfig::default(); // SVG by default
    let result = render(source, &config).unwrap();

    // Should be able to get as SVG
    let svg = result.as_svg();
    assert!(svg.is_some());
    assert!(svg.unwrap().contains("<svg"));

    // Should return None for PNG
    assert!(result.as_png().is_none());
}

#[test]
fn test_render_output_as_png() {
    let source = r#"flowchart TD
    A[Start]
"#;

    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;

    let result = render(source, &config).unwrap();

    // Should be able to get as PNG bytes
    let png = result.as_png();
    assert!(png.is_some());

    // Should return None for SVG
    assert!(result.as_svg().is_none());
}

#[test]
fn test_render_output_into_bytes() {
    let source = r#"flowchart TD
    A[Start]
"#;

    // Test SVG to bytes
    let config_svg = RenderConfig::default();
    let result_svg = render(source, &config_svg).unwrap();
    let bytes_svg = result_svg.into_bytes();
    assert!(bytes_svg.starts_with(b"<svg") || String::from_utf8_lossy(&bytes_svg).contains("<svg"));

    // Test PNG to bytes
    let mut config_png = RenderConfig::default();
    config_png.output_format = OutputFormat::Png;
    let result_png = render(source, &config_png).unwrap();
    let bytes_png = result_png.into_bytes();
    assert_eq!(
        &bytes_png[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
}

#[test]
fn test_into_svg_error() {
    let source = r#"flowchart TD
    A[Start]
"#;

    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;

    let result = render(source, &config).unwrap();

    // Trying to get SVG from PNG output should fail
    let svg_result = result.into_svg();
    assert!(svg_result.is_err());
    assert!(svg_result.unwrap_err().to_string().contains("Expected SVG"));
}

#[test]
fn test_into_png_error() {
    let source = r#"flowchart TD
    A[Start]
"#;

    let config = RenderConfig::default(); // SVG

    let result = render(source, &config).unwrap();

    // Trying to get PNG from SVG output should fail
    let png_result = result.into_png();
    assert!(png_result.is_err());
    assert!(png_result.unwrap_err().to_string().contains("Expected PNG"));
}
