use wasm_bindgen::prelude::*;
use mermaid_core::{render, RenderConfig, OutputFormat};

#[wasm_bindgen]
pub fn render_svg(source: &str) -> Result<String, JsError> {
    let config = RenderConfig::default();
    let output = render(source, &config).map_err(|e| JsError::new(&e.to_string()))?;
    output.into_svg().map_err(|e| JsError::new(&e.to_string()))
}

/// Returns PNG bytes as a Uint8Array in JS.
#[wasm_bindgen]
pub fn render_png(source: &str) -> Result<Vec<u8>, JsError> {
    let config = RenderConfig {
        output_format: OutputFormat::Png,
        ..RenderConfig::default()
    };
    let output = render(source, &config).map_err(|e| JsError::new(&e.to_string()))?;
    output.into_png().map_err(|e| JsError::new(&e.to_string()))
}
