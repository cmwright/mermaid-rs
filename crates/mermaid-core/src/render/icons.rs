/// Returns the inner SVG elements for a named icon, ready to embed in a
/// nested `<svg>` element with a 24×24 viewBox.
///
/// Source: Lucide Icons (https://lucide.dev), ISC license.
/// All icons use stroke rendering; the caller sets stroke color.
pub fn icon_paths(name: &str) -> Option<&'static str> {
    match name {
        "cloud" => Some(r#"<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>"#),
        "database" => Some(
            r#"<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>"#,
        ),
        "disk" | "hard-drive" => Some(
            r#"<path d="M10 16h.01"/><path d="M2.212 11.577a2 2 0 0 0-.212.896V18a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-5.527a2 2 0 0 0-.212-.896L18.55 5.11A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><path d="M21.946 12.013H2.054"/><path d="M6 16h.01"/>"#,
        ),
        "internet" | "globe" | "world" => Some(
            r#"<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>"#,
        ),
        "server" => Some(
            r#"<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/><rect width="20" height="8" x="2" y="14" rx="2" ry="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/>"#,
        ),
        "user" | "person" => Some(
            r#"<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>"#,
        ),
        "api" | "zap" | "lightning" => Some(
            r#"<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>"#,
        ),
        "shield" | "firewall" | "security" => Some(
            r#"<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/>"#,
        ),
        "lock" | "auth" => Some(
            r#"<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>"#,
        ),
        "cpu" | "compute" | "processor" => Some(
            r#"<path d="M12 20v2"/><path d="M12 2v2"/><path d="M17 20v2"/><path d="M17 2v2"/><path d="M2 12h2"/><path d="M2 17h2"/><path d="M2 7h2"/><path d="M20 12h2"/><path d="M20 17h2"/><path d="M20 7h2"/><path d="M7 20v2"/><path d="M7 2v2"/><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="8" y="8" width="8" height="8" rx="1"/>"#,
        ),
        "monitor" | "desktop" | "browser" => Some(
            r#"<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>"#,
        ),
        "network" | "topology" => Some(
            r#"<rect x="16" y="16" width="6" height="6" rx="1"/><rect x="2" y="16" width="6" height="6" rx="1"/><rect x="9" y="2" width="6" height="6" rx="1"/><path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"/><path d="M12 12V8"/>"#,
        ),
        "mobile" | "smartphone" | "phone" => Some(
            r#"<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>"#,
        ),
        "layers" | "stack" | "service" => Some(
            r#"<path d="M12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83z"/><path d="M2 12a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 12"/><path d="M2 17a1 1 0 0 0 .58.91l8.6 3.91a2 2 0 0 0 1.65 0l8.58-3.9A1 1 0 0 0 22 17"/>"#,
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_icons_return_some() {
        let names = [
            "cloud",
            "database",
            "disk",
            "globe",
            "server",
            "user",
            "zap",
            "shield",
            "lock",
            "cpu",
            "monitor",
            "network",
            "smartphone",
            "layers",
        ];
        for name in names {
            assert!(icon_paths(name).is_some(), "expected Some for '{name}'");
        }
    }

    #[test]
    fn aliases_return_some() {
        let aliases = [
            "hard-drive",
            "internet",
            "world",
            "person",
            "api",
            "lightning",
            "firewall",
            "security",
            "auth",
            "compute",
            "processor",
            "desktop",
            "browser",
            "topology",
            "mobile",
            "phone",
            "stack",
            "service",
        ];
        for alias in aliases {
            assert!(
                icon_paths(alias).is_some(),
                "expected Some for alias '{alias}'"
            );
        }
    }

    #[test]
    fn unknown_icon_returns_none() {
        assert!(icon_paths("notanicon").is_none());
        assert!(icon_paths("").is_none());
    }

    #[test]
    fn icon_paths_contain_svg_elements() {
        for name in ["cloud", "database", "server", "user"] {
            let paths = icon_paths(name).unwrap();
            assert!(!paths.is_empty(), "paths empty for '{name}'");
            assert!(paths.contains('<'), "no SVG elements for '{name}'");
        }
    }
}
