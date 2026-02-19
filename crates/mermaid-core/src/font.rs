use ab_glyph::FontRef;

use crate::error::{MermaidError, Result};

/// Embedded Hack font bytes (MIT License).
/// This font is bundled so the tool works out of the box without system fonts.
const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/Hack-Regular.ttf");

#[derive(Clone)]
pub struct FontProvider {
    font_data: FontData,
}

/// Internal storage for font bytes - either a static reference (for embedded fonts)
/// or an owned Vec (for user-provided fonts). This avoids copying ~240KB of embedded
/// font bytes on every FontProvider::default_font() call.
#[derive(Clone)]
enum FontData {
    Static(&'static [u8]),
    Owned(Vec<u8>),
}

impl FontData {
    fn as_slice(&self) -> &[u8] {
        match self {
            FontData::Static(s) => s,
            FontData::Owned(v) => v,
        }
    }
}

impl FontProvider {
    /// Create a FontProvider with the embedded default font.
    pub fn default_font() -> Self {
        Self {
            font_data: FontData::Static(DEFAULT_FONT_BYTES),
        }
    }

    /// Create a FontProvider from custom TTF/OTF font bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        // Validate the font data parses successfully
        FontRef::try_from_slice(&data)
            .map_err(|e| MermaidError::Font(format!("Invalid font data: {}", e)))?;
        Ok(Self {
            font_data: FontData::Owned(data),
        })
    }

    /// Get a FontRef for measurement operations.
    pub fn font_ref(&self) -> Result<FontRef<'_>> {
        FontRef::try_from_slice(self.font_data.as_slice())
            .map_err(|e| MermaidError::Font(format!("Failed to load font: {}", e)))
    }

    /// Get the raw font data.
    pub fn font_data(&self) -> Vec<u8> {
        self.font_data.as_slice().to_vec()
    }
}

impl Default for FontProvider {
    fn default() -> Self {
        Self::default_font()
    }
}

impl std::fmt::Debug for FontProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontProvider")
            .field("font_data_len", &self.font_data.as_slice().len())
            .finish()
    }
}
