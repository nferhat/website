//! # `fonts` - font subsetting
//!
//! This makes use of [`allsorts`] in order to subset the fonts into smaller families, in order to
//! reduce the total bundle size needed to load the website.

use allsorts::{
    binary::read::ReadScope,
    error::ParseError,
    font::read_cmap_subtable,
    font_data::FontData,
    get_name::fontcode_get_name,
    gsub::{GlyphOrigin, RawGlyph, RawGlyphFlags},
    subset::{CmapTarget, SubsetProfile},
    tables::{
        FontTableProvider,
        cmap::{Cmap, CmapSubtable},
        os2::Os2,
    },
    tag,
    tinyvec::tiny_vec,
};
use eyre::{Context, ContextCompat, Result, bail};

/// Subsets the given loaded font bytes with the given HTML content. Returns the subsetted font bytes
pub fn subset(font_bytes: &[u8], chars: impl Iterator<Item = char>) -> Result<Vec<u8>> {
    let font_file = ReadScope::new(font_bytes)
        .read::<FontData>()
        .context("failed to read font")?;
    let font_provider = font_file.table_provider(0)?; // XXX: Which index to use?
    // FIXME: For now we subset for ALL html which could include some additional stuff
    // that might not really be needed in the font, but I digress.
    let res = subset_text(&font_provider, chars)?;
    Ok(res)
}

fn subset_text<F, Chars>(font_provider: &F, chars: Chars) -> Result<Vec<u8>>
where
    F: FontTableProvider,
    Chars: Iterator<Item = char>,
{
    // Work out the glyphs we want to keep from the text
    let mut glyphs = chars_to_glyphs(font_provider, chars)?;
    let notdef = RawGlyph {
        unicodes: tiny_vec![],
        glyph_index: 0,
        liga_component_pos: 0,
        glyph_origin: GlyphOrigin::Direct,
        flags: RawGlyphFlags::empty(),
        variation: None,
        extra_data: (),
    };
    glyphs.insert(0, Some(notdef));

    let mut glyphs: Vec<RawGlyph<()>> = glyphs.into_iter().flatten().collect();
    glyphs.sort_by(|a, b| a.glyph_index.cmp(&b.glyph_index));
    let mut glyph_ids = glyphs
        .iter()
        .map(|glyph| glyph.glyph_index)
        .collect::<Vec<_>>();
    glyph_ids.dedup();
    if glyph_ids.is_empty() {
        bail!("no glyphs left in font")
    }

    // Subset
    let profile = SubsetProfile::Minimal;
    let cmap_target = CmapTarget::Unicode;
    let new_font = allsorts::subset::subset(font_provider, &glyph_ids, &profile, cmap_target)?;

    Ok(new_font)
}

fn chars_to_glyphs<F, Chars>(font_provider: &F, chars: Chars) -> Result<Vec<Option<RawGlyph<()>>>>
where
    F: FontTableProvider,
    Chars: Iterator<Item = char>,
{
    let cmap_data = font_provider.read_table_data(tag::CMAP)?;
    let cmap = ReadScope::new(&cmap_data).read::<Cmap>()?;
    let (_, cmap_subtable) = read_cmap_subtable(&cmap)?.context("failed to read CMAP table")?;

    let glyphs = chars
        .map(|ch| map_char(&cmap_subtable, ch))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(glyphs)
}

fn map_char(cmap_subtable: &CmapSubtable, ch: char) -> Result<Option<RawGlyph<()>>, ParseError> {
    if let Some(glyph_index) = cmap_subtable.map_glyph(ch as u32)? {
        let glyph = make_raw_glyph(ch, glyph_index);
        Ok(Some(glyph))
    } else {
        Ok(None)
    }
}

fn make_raw_glyph(ch: char, glyph_index: u16) -> RawGlyph<()> {
    RawGlyph {
        unicodes: tiny_vec![[char; 1] => ch],
        glyph_index,
        liga_component_pos: 0,
        glyph_origin: GlyphOrigin::Char(ch),
        flags: RawGlyphFlags::empty(),
        variation: None,
        extra_data: (),
    }
}

/// Represents the style of a font file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    /// The family name (name ID 1)
    pub family: String,
    /// The subfamily name (name ID 2) — e.g. "Regular", "Bold", "Italic"
    pub subfamily: String,
    /// Numeric weight (400 = Regular, 700 = Bold)
    pub weight: u16,
    /// Whether this is an italic/oblique font
    pub is_italic: bool,
    /// Whether this is a bold font
    pub is_bold: bool,
}

/// Extracts the full style info (family, subfamily, weight, italic/bold) from a font.
pub fn font_style(font_bytes: &[u8]) -> Result<Style> {
    let font_scope = ReadScope::new(font_bytes);
    let font_file = font_scope
        .read::<FontData>()
        .context("failed to read font")?;
    let font_provider = font_file.table_provider(0)?; // XXX: Which index to use?
    let family = read_name(&font_provider, 1)
        .or_else(|| read_name(&font_provider, 16))
        .unwrap_or_else(|| "CustomFont".to_string());

    let subfamily = read_name(&font_provider, 2).unwrap_or_else(|| "Regular".to_string());
    let os2 = font_provider
        .table_data(tag::OS_2)?
        .map(|data| ReadScope::new(&data).read_dep::<Os2>(data.len()))
        .transpose()
        .context("failed to read OS_2 table")?
        .context("missing OS_2 table")?;

    let weight = os2.us_weight_class;
    let fs = os2.fs_selection;
    let is_italic = fs.contains(allsorts::tables::os2::FsSelection::ITALIC);
    let is_bold = fs.contains(allsorts::tables::os2::FsSelection::BOLD);

    Ok(Style {
        family,
        subfamily,
        weight,
        is_italic,
        is_bold,
    })
}

fn read_name<F: FontTableProvider>(font_provider: &F, name_id: u16) -> Option<String> {
    font_provider
        .read_table_data(tag::NAME)
        .ok()
        .and_then(|data| fontcode_get_name(&data, name_id).ok())
        .flatten()
        .map(|s| s.to_string_lossy().into_owned())
}
