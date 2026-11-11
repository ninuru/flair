use std::{cell::OnceCell, fs, path::PathBuf, rc::Rc, sync::Arc};

use crossfont::ft::fc;
use freetype::{
    LcdFilter, Library, RenderMode,
    bitmap::{Bitmap, PixelMode},
    face::LoadFlag,
    ffi::FT_GLYPH_FORMAT_BITMAP,
};

use crate::{
    cellgrid::DrawCell,
    font::{RasterGlyph, Style},
    shape::{ShapeCache, Shaped},
};

pub struct ShapeSource {
    size: f32,
    faces: [Option<FaceSource>; 4],
}

struct FaceSource {
    path: PathBuf,
    index: isize,
    load_flags: LoadFlag,
    render_mode: RenderMode,
    lcd_filter: LcdFilter,
}

impl ShapeSource {
    pub fn locate(family: &str, size: f32) -> ShapeSource {
        let config = fc::Config::get_current();
        let faces = [
            (fc::Slant::Roman, fc::Weight::Regular),
            (fc::Slant::Roman, fc::Weight::Bold),
            (fc::Slant::Italic, fc::Weight::Regular),
            (fc::Slant::Italic, fc::Weight::Bold),
        ]
        .map(|(slant, weight)| face_source(config, family, size, slant, weight));
        ShapeSource { size, faces }
    }
}

fn face_source(
    config: &fc::ConfigRef,
    family: &str,
    size: f32,
    slant: fc::Slant,
    weight: fc::Weight,
) -> Option<FaceSource> {
    let mut pattern = fc::Pattern::new();
    pattern.add_family(family);
    pattern.add_pixelsize(f64::from(size));
    pattern.set_weight(weight);
    pattern.set_slant(slant);
    pattern.config_substitute(config, fc::MatchKind::Pattern);
    pattern.default_substitute();
    let font = fc::font_match(config, &pattern)?;
    let synthetic = font.embolden().next().unwrap_or(false) || font.get_matrix().is_some();
    let bitmap = font.color().next().unwrap_or(false) || !font.scalable().next().unwrap_or(true);
    if synthetic || bitmap {
        return None;
    }
    let location = font.ft_face_location(0)?;
    Some(FaceSource {
        path: location.path,
        index: location.index,
        load_flags: load_flags(&font),
        render_mode: render_mode(&font),
        lcd_filter: match font.lcdfilter().next().unwrap_or(fc::LcdFilter::Default) {
            fc::LcdFilter::None => LcdFilter::LcdFilterNone,
            fc::LcdFilter::Default => LcdFilter::LcdFilterDefault,
            fc::LcdFilter::Light => LcdFilter::LcdFilterLight,
            fc::LcdFilter::Legacy => LcdFilter::LcdFilterLegacy,
        },
    })
}

fn load_flags(font: &fc::PatternRef) -> LoadFlag {
    let antialias = font.antialias().next().unwrap_or(true);
    let hintstyle = if font.hinting().next().unwrap_or(true) {
        font.hintstyle().next().unwrap_or(fc::HintStyle::Full)
    } else {
        fc::HintStyle::None
    };
    let rgba = font.rgba().next().unwrap_or(fc::Rgba::Unknown);
    let mut flags = match (antialias, hintstyle, rgba) {
        (false, fc::HintStyle::None, _) => LoadFlag::NO_HINTING | LoadFlag::MONOCHROME,
        (false, ..) => LoadFlag::TARGET_MONO | LoadFlag::MONOCHROME,
        (true, fc::HintStyle::None, _) => LoadFlag::NO_HINTING,
        (true, fc::HintStyle::Slight, _) => LoadFlag::TARGET_LIGHT,
        (true, fc::HintStyle::Medium, _) => LoadFlag::TARGET_NORMAL,
        (true, fc::HintStyle::Full, fc::Rgba::Rgb | fc::Rgba::Bgr) => LoadFlag::TARGET_LCD,
        (true, fc::HintStyle::Full, fc::Rgba::Vrgb | fc::Rgba::Vbgr) => LoadFlag::TARGET_LCD_V,
        (true, fc::HintStyle::Full, fc::Rgba::Unknown | fc::Rgba::None) => LoadFlag::TARGET_NORMAL,
    };
    if !font.embeddedbitmap().next().unwrap_or(true) {
        flags |= LoadFlag::NO_BITMAP;
    }
    if font.autohint().next().unwrap_or(false) {
        flags |= LoadFlag::FORCE_AUTOHINT;
    }
    flags
}

fn render_mode(font: &fc::PatternRef) -> RenderMode {
    match (
        font.antialias().next().unwrap_or(true),
        font.rgba().next().unwrap_or(fc::Rgba::Unknown),
    ) {
        (false, _) => RenderMode::Mono,
        (true, fc::Rgba::Rgb | fc::Rgba::Bgr) => RenderMode::Lcd,
        (true, fc::Rgba::Vrgb | fc::Rgba::Vbgr) => RenderMode::LcdV,
        (true, fc::Rgba::Unknown | fc::Rgba::None) => RenderMode::Normal,
    }
}

struct ShapedFace {
    bytes: Rc<Vec<u8>>,
    index: u32,
    face: freetype::Face,
    load_flags: LoadFlag,
    render_mode: RenderMode,
    lcd_filter: LcdFilter,
}

impl ShapedFace {
    fn open(library: &Library, source: &FaceSource, size: f32) -> Option<ShapedFace> {
        let path = source.path.display();
        let bytes = fs::read(&source.path)
            .inspect_err(|error| log::warn!("ligatures: {path}: {error}"))
            .ok()?;
        let bytes = Rc::new(bytes);
        let index = u32::try_from(source.index).ok()?;
        rustybuzz::Face::from_slice(&bytes, index)?;
        let face = library
            .new_memory_face(Rc::clone(&bytes), source.index)
            .inspect_err(|error| log::warn!("ligatures: {path}: {error}"))
            .ok()?;
        face.set_char_size((size * 64.0).round() as isize, 0, 0, 0)
            .ok()?;
        Some(ShapedFace {
            bytes,
            index,
            face,
            load_flags: source.load_flags,
            render_mode: source.render_mode,
            lcd_filter: source.lcd_filter,
        })
    }

    fn parse(&self) -> Option<rustybuzz::Face<'_>> {
        rustybuzz::Face::from_slice(&self.bytes, self.index)
    }

    fn rasterize(&self, library: &Library, id: u16) -> Option<RasterGlyph> {
        library.set_lcd_filter(self.lcd_filter).ok()?;
        self.face.load_glyph(u32::from(id), self.load_flags).ok()?;
        let slot = self.face.glyph();
        if slot.raw().format != FT_GLYPH_FORMAT_BITMAP {
            slot.render_glyph(self.render_mode).ok()?;
        }
        let (width, height, pixels) = coverage(&slot.bitmap())?;
        if width == 0 || height == 0 {
            return None;
        }
        Some(RasterGlyph {
            width,
            height,
            left: slot.bitmap_left(),
            top: slot.bitmap_top(),
            colored: false,
            pixels,
        })
    }
}

fn shape(face: &rustybuzz::Face, text: &str) -> Vec<Option<Shaped>> {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    let glyphs = rustybuzz::shape(face, &[], buffer);
    let infos = glyphs.glyph_infos();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let aligned = infos.len() == chars.len()
        && infos
            .iter()
            .zip(&chars)
            .all(|(info, (offset, _))| info.cluster as usize == *offset);
    if !aligned {
        return Vec::new();
    }
    chars
        .iter()
        .zip(infos)
        .map(|((_, c), info)| {
            let id = u16::try_from(info.glyph_id).ok()?;
            let plain = face.glyph_index(*c).map(|glyph| glyph.0);
            let blank = face
                .glyph_bounding_box(rustybuzz::ttf_parser::GlyphId(id))
                .is_none();
            (id != 0 && plain != Some(id)).then_some(if blank {
                Shaped::Blank
            } else {
                Shaped::Glyph(id)
            })
        })
        .collect()
}

fn coverage(bitmap: &Bitmap) -> Option<(u32, u32, Vec<u8>)> {
    let buffer = bitmap.buffer();
    let pitch = bitmap.pitch().unsigned_abs() as usize;
    let rows = usize::try_from(bitmap.rows()).ok()?;
    let width = usize::try_from(bitmap.width()).ok()?;
    let alpha = |alpha: u8| [255, 255, 255, alpha];
    let mean = |samples: [u8; 3]| (samples.iter().map(|&s| u16::from(s)).sum::<u16>() / 3) as u8;
    let (w, h, pixels): (usize, usize, Vec<u8>) = match bitmap.pixel_mode().ok()? {
        PixelMode::Gray => (
            width,
            rows,
            (0..rows)
                .flat_map(|y| buffer[y * pitch..y * pitch + width].iter().copied())
                .flat_map(alpha)
                .collect(),
        ),
        PixelMode::Lcd => (
            width / 3,
            rows,
            (0..rows)
                .flat_map(|y| {
                    buffer[y * pitch..y * pitch + width]
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .map(|p| mean(*p))
                })
                .flat_map(alpha)
                .collect(),
        ),
        PixelMode::LcdV => (
            width,
            rows / 3,
            (0..rows / 3)
                .flat_map(|y| {
                    (0..width)
                        .map(move |x| mean([0, 1, 2].map(|k| buffer[(y * 3 + k) * pitch + x])))
                })
                .flat_map(alpha)
                .collect(),
        ),
        PixelMode::Mono => (
            width,
            rows,
            (0..rows)
                .flat_map(|y| {
                    (0..width).map(move |x| ((buffer[y * pitch + x / 8] >> (7 - x % 8)) & 1) * 255)
                })
                .flat_map(alpha)
                .collect(),
        ),
        PixelMode::None | PixelMode::Gray2 | PixelMode::Gray4 | PixelMode::Bgra => return None,
    };
    Some((u32::try_from(w).ok()?, u32::try_from(h).ok()?, pixels))
}

fn slot(style: Style) -> usize {
    match style {
        Style::Regular => 0,
        Style::Bold => 1,
        Style::Italic => 2,
        Style::BoldItalic => 3,
    }
}

struct Faces {
    source: Arc<ShapeSource>,
    library: Option<Library>,
    faces: [OnceCell<Option<ShapedFace>>; 4],
}

impl Faces {
    fn face(&self, style: Style) -> Option<(&Library, &ShapedFace)> {
        let library = self.library.as_ref()?;
        let index = slot(style);
        let face = self.faces[index]
            .get_or_init(|| {
                ShapedFace::open(
                    library,
                    self.source.faces[index].as_ref()?,
                    self.source.size,
                )
            })
            .as_ref()?;
        Some((library, face))
    }
}

pub struct Shaper {
    faces: Faces,
    cache: ShapeCache,
}

impl Shaper {
    pub fn new(source: Arc<ShapeSource>) -> Shaper {
        let library = Library::init()
            .inspect_err(|error| log::warn!("ligatures: freetype: {error}"))
            .ok();
        Shaper {
            faces: Faces {
                source,
                library,
                faces: Default::default(),
            },
            cache: ShapeCache::default(),
        }
    }

    pub fn serves(&self, source: &Arc<ShapeSource>) -> bool {
        Arc::ptr_eq(&self.faces.source, source)
    }

    pub fn row(&mut self, cells: &[DrawCell], cursor: Option<usize>) -> Vec<Option<Shaped>> {
        let faces = &self.faces;
        let mut parsed: [Option<Option<rustybuzz::Face>>; 4] = Default::default();
        self.cache.row(cells, cursor, |text, style| {
            let Some((_, face)) = faces.face(style) else {
                return Vec::new();
            };
            parsed[slot(style)]
                .get_or_insert_with(|| face.parse())
                .as_ref()
                .map_or_else(Vec::new, |parsed| shape(parsed, text))
        })
    }

    pub fn rasterize(&self, id: u16, style: Style) -> Option<RasterGlyph> {
        let (library, face) = self.faces.face(style)?;
        face.rasterize(library, id)
    }
}

#[cfg(test)]
#[path = "shaper_tests.rs"]
mod tests;
