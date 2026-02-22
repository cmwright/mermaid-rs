use mermaid_core::render::theme::Theme;
use mermaid_core::{render, RenderConfig};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct RenderResult {
    pub svg: Option<String>,
    pub error: Option<String>,
}

#[wasm_bindgen]
pub fn render_svg_with_theme(source: &str, theme_name: &str) -> JsValue {
    let theme = Theme::by_name(theme_name);
    let config = RenderConfig {
        theme,
        ..RenderConfig::default()
    };

    match render(source, &config) {
        Ok(output) => match output.into_svg() {
            Ok(svg) => {
                let result = RenderResult {
                    svg: Some(svg),
                    error: None,
                };
                serde_wasm_bindgen::to_value(&result).unwrap()
            }
            Err(e) => {
                let result = RenderResult {
                    svg: None,
                    error: Some(e.to_string()),
                };
                serde_wasm_bindgen::to_value(&result).unwrap()
            }
        },
        Err(e) => {
            let result = RenderResult {
                svg: None,
                error: Some(e.to_string()),
            };
            serde_wasm_bindgen::to_value(&result).unwrap()
        }
    }
}

#[wasm_bindgen]
pub fn render_svg(source: &str) -> JsValue {
    render_svg_with_theme(source, "default")
}

#[wasm_bindgen]
pub fn get_supported_themes() -> JsValue {
    let themes = vec!["default", "dark", "forest", "neutral"];
    serde_wasm_bindgen::to_value(&themes).unwrap()
}
