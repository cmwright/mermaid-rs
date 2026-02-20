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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_font_and_font_ref() {
        let provider = FontProvider::default_font();
        let font_ref = provider.font_ref();
        assert!(font_ref.is_ok(), "font_ref() should succeed for default font");
    }

    #[test]
    fn from_bytes_invalid_returns_err() {
        let bad_data = vec![0u8, 1, 2, 3];
        let result = FontProvider::from_bytes(bad_data);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("Font error"), "error should mention Font error: {}", msg);
        assert!(msg.contains("Invalid font data"), "error should mention Invalid font data: {}", msg);
    }

    #[test]
    fn font_data_returns_non_empty() {
        let provider = FontProvider::default_font();
        let data = provider.font_data();
        assert!(!data.is_empty(), "font_data() should return non-empty bytes");
    }

    #[test]
    fn debug_formatting() {
        let provider = FontProvider::default_font();
        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("FontProvider"), "Debug output should contain 'FontProvider'");
        assert!(debug_str.contains("font_data_len"), "Debug output should contain 'font_data_len'");
    }

    #[test]
    fn default_impl() {
        let provider = FontProvider::default();
        // Default impl delegates to default_font(), so font_ref should work
        assert!(provider.font_ref().is_ok());
    }

    #[test]
    fn from_bytes_with_valid_data_owned_path() {
        // Get the raw bytes from the default (Static) provider, then create a new
        // provider via from_bytes which exercises the Owned variant of FontData.
        let default_provider = FontProvider::default_font();
        let raw_bytes = default_provider.font_data();

        let owned_provider = FontProvider::from_bytes(raw_bytes)
            .expect("from_bytes should succeed with valid font data");

        // Verify the Owned-backed provider works end-to-end
        let font_ref = owned_provider.font_ref();
        assert!(font_ref.is_ok(), "font_ref() should succeed for Owned font data");

        let data = owned_provider.font_data();
        assert!(!data.is_empty(), "font_data() on Owned provider should return non-empty bytes");

        // Debug should still work
        let debug_str = format!("{:?}", owned_provider);
        assert!(debug_str.contains("font_data_len"));
    }
}
