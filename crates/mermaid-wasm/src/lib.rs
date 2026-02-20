use wasm_bindgen::prelude::*;
use mermaid_core::{render, RenderConfig};

#[wasm_bindgen]
pub fn render_svg(source: &str) -> Result<String, JsError> {
    let config = RenderConfig::default();
    let output = render(source, &config).map_err(|e| JsError::new(&e.to_string()))?;
    output.into_svg().map_err(|e| JsError::new(&e.to_string()))
}
