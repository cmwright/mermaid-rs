use ab_glyph::FontRef;

use crate::error::{MermaidError, Result};

/// Embedded DejaVu Sans font bytes (SIL Open Font License).
/// This font is bundled so the tool works out of the box without system fonts.
const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

#[derive(Clone)]
pub struct FontProvider {
    font_data: Vec<u8>,
}

impl FontProvider {
    /// Create a FontProvider with the embedded default font.
    pub fn default_font() -> Self {
        Self {
            font_data: DEFAULT_FONT_BYTES.to_vec(),
        }
    }

    /// Create a FontProvider from custom TTF/OTF font bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        // Validate the font data parses successfully
        FontRef::try_from_slice(&data)
            .map_err(|e| MermaidError::Font(format!("Invalid font data: {}", e)))?;
        Ok(Self { font_data: data })
    }

    /// Get a FontRef for measurement operations.
    pub fn font_ref(&self) -> Result<FontRef<'_>> {
        FontRef::try_from_slice(&self.font_data)
            .map_err(|e| MermaidError::Font(format!("Failed to load font: {}", e)))
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
            .field("font_data_len", &self.font_data.len())
            .finish()
    }
}
