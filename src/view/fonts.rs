//! The two font roles share loading and retain independent raster caches.
use anyhow::{anyhow, Result};
use fontdue::{Font, FontSettings};

pub(super) struct Fonts {
    pub editor: Font,
    pub ui: Font,
}

impl Fonts {
    pub fn load(editor: &str, ui: &str) -> Result<Self> {
        let mut database = None;
        Ok(Self {
            editor: load_family(editor, true, &mut database)?,
            ui: load_family(ui, false, &mut database)?,
        })
    }
}

fn embedded(name: &str) -> Option<&'static [u8]> {
    match name {
        "JetBrains Mono" => Some(include_bytes!("../../assets/JetBrainsMono.ttf")),
        "Inter" => Some(include_bytes!("../../assets/Inter-Regular.ttf")),
        _ => None,
    }
}

fn load_family(name: &str, editor: bool, database: &mut Option<fontdb::Database>) -> Result<Font> {
    let fallback = if editor { "JetBrains Mono" } else { "Inter" };
    let name = name.trim();
    let loaded = if name.is_empty() {
        Err(anyhow!("font family is empty"))
    } else if let Some(bytes) = embedded(name) {
        if editor && name == "Inter" {
            Err(anyhow!("editor_font must be monospaced"))
        } else {
            Font::from_bytes(bytes, FontSettings::default()).map_err(|e| anyhow!(e))
        }
    } else {
        let db = database.get_or_insert_with(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        });
        load_installed(db, name, editor)
    };
    match loaded {
        Ok(font) => Ok(font),
        Err(error) => {
            tracing::warn!(%name, %fallback, %error, "Cannot use configured font; using bundled fallback");
            Font::from_bytes(
                embedded(fallback).expect("both fallback fonts are embedded"),
                FontSettings::default(),
            )
            .map_err(|e| anyhow!(e))
        }
    }
}

fn load_installed(db: &fontdb::Database, name: &str, editor: bool) -> Result<Font> {
    let id = db
        .query(&fontdb::Query {
            families: &[fontdb::Family::Name(name)],
            ..Default::default()
        })
        .ok_or_else(|| anyhow!("font family is not installed"))?;
    if editor && !db.face(id).is_some_and(|face| face.monospaced) {
        return Err(anyhow!("editor_font must be monospaced"));
    }
    db.with_face_data(id, |bytes, collection_index| {
        Font::from_bytes(
            bytes,
            FontSettings {
                collection_index,
                ..Default::default()
            },
        )
        .map_err(|e| anyhow!(e))
    })
    .ok_or_else(|| anyhow!("font file could not be read"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{FontRole, GlyphCache, TextPainter};

    #[test]
    fn font_roles_keep_code_metrics_and_ui_caches_independent() {
        let fonts = Fonts::load("JetBrains Mono", "Inter").unwrap();
        let code_width = fonts.editor.metrics('M', 14.0).advance_width;
        assert_eq!(fonts.editor.metrics('i', 14.0).advance_width, code_width);
        assert!(
            fonts.ui.metrics('W', 14.0).advance_width > fonts.ui.metrics('i', 14.0).advance_width
        );
        let mut code_cache = GlyphCache::new();
        let mut ui_cache = GlyphCache::new();
        let mut painter =
            TextPainter::new(&fonts.editor, &mut code_cache, 14.0, 11.0, code_width, 20)
                .with_ui_font(&fonts.ui, &mut ui_cache, FontRole::Ui);
        let ui_width = painter.measure_width("iiii");
        assert_eq!(painter.char_width(), code_width);
        {
            let mut code = painter.with_font(FontRole::Code);
            assert_eq!(code.measure_width("iiii"), code_width * 4.0);
            {
                let mut ui = code.with_font(FontRole::Ui);
                assert_eq!(ui.measure_width("iiii"), ui_width);
            }
            assert_eq!(code.font_role(), FontRole::Code);
            assert_eq!(code.measure_width("iiii"), code_width * 4.0);
        }
        assert_eq!(painter.font_role(), FontRole::Ui);
        assert_eq!(painter.measure_width("iiii"), ui_width);
        assert!(ui_width < code_width * 4.0);
        let clipped = painter.truncate_to_width("WWiiiiWW", 35.0);
        assert!(painter.measure_width(&clipped) <= 35.0);
        assert!(matches!(
            painter.truncate_to_width("short", 100.0),
            std::borrow::Cow::Borrowed(_)
        ));
        assert_eq!(painter.truncate_to_width("anything", 0.0), "");
        let unicode = painter.truncate_to_width("café_very_long_name", 45.0);
        assert!(unicode.ends_with('…'));
        assert!(painter.measure_width(&unicode) <= 45.0);

        let rejected = Fonts::load("Inter", "JetBrains Mono").unwrap();
        assert_eq!(rejected.editor.metrics('i', 14.0).advance_width, code_width);
        assert_eq!(rejected.ui.metrics('i', 14.0).advance_width, code_width);
    }
}
