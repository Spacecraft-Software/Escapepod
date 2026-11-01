// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Human-mode colour through the Steelbore §11.1 role tokens.
//!
//! The values are never typed here: they are read from
//! `assets/steelbore.toml`, the palette family's canonical contract
//! (Standard §11.4), vendored unchanged from the `steelbore-color-palette`
//! bundle. The default theme is `steelbore`; `SPACECRAFT_THEME` selects
//! another, and accessible mode selects its `-high-contrast` sibling.

use std::collections::BTreeMap;

/// The palette contract, vendored unchanged.
const PALETTE: &str = include_str!("../assets/steelbore.toml");

/// The §11.1 role tokens Escape Pod's human output uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    /// `[ERROR]` tags.
    Error,
    /// `[WARN]` tags.
    Warning,
    /// `[OK]` tags.
    Success,
    /// `[INFO]` tags and headings.
    Structure,
    /// `hint:` labels and managed keys.
    Accent,
}

impl Token {
    fn key(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Success => "success",
            Self::Structure => "structure",
            Self::Accent => "accent",
        }
    }
}

/// One resolved theme: an SGR prefix per token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    slug: String,
    sgr: BTreeMap<&'static str, String>,
}

impl Theme {
    /// Resolves `slug` (or `steelbore`), taking the high-contrast sibling
    /// in accessible mode when the palette has one.
    ///
    /// # Errors
    ///
    /// Returns the slug when the palette has no such theme.
    pub fn resolve(slug: Option<&str>, accessible: bool) -> Result<Self, String> {
        let base = slug.unwrap_or("steelbore");
        let themes = themes();
        let chosen = if accessible && !base.ends_with("-high-contrast") && base != "steelbore-mono"
        {
            let hc = format!("{base}-high-contrast");
            if themes.contains_key(&hc) {
                hc
            } else {
                base.to_owned()
            }
        } else {
            base.to_owned()
        };
        let table = themes.get(&chosen).ok_or_else(|| base.to_owned())?;
        let tokens = [
            Token::Error,
            Token::Warning,
            Token::Success,
            Token::Structure,
            Token::Accent,
        ];
        let sgr = tokens
            .iter()
            .filter_map(|t| {
                let v = table.get(t.key())?.as_str()?;
                Some((t.key(), sgr(v)?))
            })
            .collect();
        Ok(Self { slug: chosen, sgr })
    }

    /// The default theme, or its high-contrast sibling.
    pub fn fallback(accessible: bool) -> Self {
        Self::resolve(None, accessible).unwrap_or(Self {
            slug: "steelbore".into(),
            sgr: BTreeMap::new(),
        })
    }

    /// The theme's slug.
    #[cfg(test)]
    pub fn slug(&self) -> &str {
        &self.slug
    }

    /// `text` wrapped in the token's colour and a reset.
    pub fn paint(&self, token: Token, text: &str) -> String {
        match self.sgr.get(token.key()) {
            Some(p) => format!("\x1b[{p}m{text}\x1b[0m"),
            None => text.to_owned(),
        }
    }
}

fn themes() -> toml::Table {
    toml::from_str::<toml::Table>(PALETTE)
        .ok()
        .and_then(|t| t.get("themes").and_then(toml::Value::as_table).cloned())
        .unwrap_or_default()
}

/// The SGR parameters for a palette value: `#RRGGBB` as 24-bit colour, or
/// a `steelbore-mono` name as a basic ANSI colour.
fn sgr(value: &str) -> Option<String> {
    if let Some(hex) = value.strip_prefix('#') {
        let n = u32::from_str_radix(hex, 16).ok()?;
        return Some(format!(
            "38;2;{};{};{}",
            (n >> 16) & 0xff,
            (n >> 8) & 0xff,
            n & 0xff
        ));
    }
    let code = match value {
        "red" => "31",
        "green" => "32",
        "yellow" => "33",
        "blue" => "34",
        "bright-white" => "97",
        "reverse-video" => "7",
        _ => return None,
    };
    Some(code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_accessible_themes_resolve() {
        let t = Theme::resolve(None, false).unwrap_or_else(|_| Theme::fallback(false));
        assert_eq!(t.slug(), "steelbore");
        assert!(t.paint(Token::Error, "x").starts_with("\x1b[38;2;"));
        let hc = Theme::resolve(None, true).unwrap_or_else(|_| Theme::fallback(false));
        assert_eq!(hc.slug(), "steelbore-high-contrast");
        assert_ne!(t.paint(Token::Error, "x"), hc.paint(Token::Error, "x"));
        let mono =
            Theme::resolve(Some("steelbore-mono"), true).unwrap_or_else(|_| Theme::fallback(false));
        assert_eq!(mono.paint(Token::Error, "x"), "\x1b[31mx\x1b[0m");
        assert_eq!(
            Theme::resolve(Some("no-such-theme"), false),
            Err("no-such-theme".to_owned())
        );
    }
}

// Rust guideline compliant 2026-05-18
