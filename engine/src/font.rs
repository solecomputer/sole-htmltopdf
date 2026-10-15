use read_fonts::TableProvider;
use unicode_script::UnicodeScript;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceRequest {
    pub(crate) rule: crate::css::FaceRule,
    pub(crate) base: Option<std::path::PathBuf>,
    pub(crate) origin: String,
}

#[derive(Debug, Clone)]
pub(crate) struct FaceEntry {
    pub(crate) id: FontId,
    pub(crate) family: String,
    pub(crate) weight: crate::contracts::FontWeight,
    pub(crate) style: crate::contracts::FontStyle,
    pub(crate) bytes: std::sync::Arc<[u8]>,
    pub(crate) units_per_em: u16,
    pub(crate) ascent: i16,
    pub(crate) descent: i16,
    pub(crate) y_min: i16,
    pub(crate) y_max: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FontId(pub(crate) usize);

#[derive(Debug, Clone)]
pub(crate) struct AuthoredFace {
    pub(crate) id: FontId,
    bytes: std::sync::Arc<[u8]>,
    units_per_em: u16,
    ascent: i16,
    descent: i16,
    y_min: i16,
    y_max: i16,
    shaping: std::sync::Arc<std::sync::Mutex<ShapingState>>,
}

impl PartialEq for AuthoredFace {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionedRun {
    pub(crate) face: FontId,
    pub(crate) source: String,
    pub(crate) glyphs: Vec<PositionedGlyph>,
    pub(crate) advance: f32,
    pub(crate) ink_above: f32,
    pub(crate) ink_below: f32,
}

impl PositionedRun {
    pub(crate) fn append(&mut self, mut next: Self) {
        debug_assert_eq!(self.face, next.face);
        let offset = u32::try_from(self.source.len()).unwrap_or(u32::MAX);
        for glyph in &mut next.glyphs {
            glyph.cluster = glyph.cluster.saturating_add(offset);
        }
        self.source.push_str(&next.source);
        self.glyphs.extend(next.glyphs);
        self.advance += next.advance;
        self.ink_above = self.ink_above.max(next.ink_above);
        self.ink_below = self.ink_below.max(next.ink_below);
    }

    pub(crate) fn scaled(mut self, factor: f32) -> Self {
        self.advance *= factor;
        self.ink_above *= factor;
        self.ink_below *= factor;
        for glyph in &mut self.glyphs {
            glyph.x_advance *= factor;
            glyph.x_offset *= factor;
            glyph.y_offset *= factor;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PositionedGlyph {
    pub(crate) id: u16,
    pub(crate) cluster: u32,
    pub(crate) x_advance: f32,
    pub(crate) x_offset: f32,
    pub(crate) y_offset: f32,
}

#[derive(Debug)]
pub(crate) struct FontTable {
    entries: Vec<FaceEntry>,
    attempts: usize,
    accepted_bytes: usize,
    shaping: std::sync::Arc<std::sync::Mutex<ShapingState>>,
}

impl Default for FontTable {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            attempts: 0,
            accepted_bytes: 0,
            shaping: std::sync::Arc::new(std::sync::Mutex::new(ShapingState::default())),
        }
    }
}

#[derive(Debug, Default)]
struct ShapingState {
    calls: usize,
    scalars: usize,
    operations: u64,
    capacity_units: u64,
    retained_glyphs: usize,
    cache: std::collections::BTreeMap<(usize, u32, String), PositionedRun>,
    refused_keys: std::collections::BTreeSet<(usize, u32, String)>,
    refused: usize,
    first_refused: Option<String>,
    context_splits: usize,
    first_split: Option<String>,
    oversized_graphemes: usize,
    first_oversized_grapheme: Option<String>,
}

const MAX_SHAPING_CALLS: usize = 4_096;

const MAX_SHAPING_SCALARS: usize = 65_536;

const MAX_SHAPING_OPERATIONS: u64 = 268_435_456;

const MAX_SHAPING_CAPACITY_UNITS: u64 = 67_108_864;

const MAX_RETAINED_GLYPHS: usize = 262_144;

pub(crate) const MAX_FACES: usize = 16;

pub(crate) const MAX_ATTEMPTS: usize = 64;

pub(crate) const MAX_FACE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
struct ValidatedFace {
    bytes: Vec<u8>,
    units_per_em: u16,
    ascent: i16,
    descent: i16,
    y_min: i16,
    y_max: i16,
}

impl FontTable {
    pub(crate) fn acquire(
        requests: Vec<FaceRequest>,
        host: &dyn crate::contracts::RenderHost,
        warnings: &mut Vec<crate::contracts::Warning>,
    ) -> Self {
        let mut table = Self::default();
        let mut refused = Refusals::default();
        let mut face_limit = false;
        let mut attempt_limit = false;
        let mut byte_limit = false;
        for request in requests {
            if table.entries.len() >= MAX_FACES {
                face_limit = true;
                refused.record(format!(
                    "{}: the {MAX_FACES}-face limit was reached",
                    request.rule.family
                ));
                break;
            }
            let mut accepted = false;
            for source in &request.rule.sources {
                if table.attempts >= MAX_ATTEMPTS {
                    attempt_limit = true;
                    refused.record(format!(
                        "{}: the {MAX_ATTEMPTS}-attempt limit was reached",
                        request.rule.family
                    ));
                    break;
                }
                table.attempts = table.attempts.saturating_add(1);
                if table.accepted_bytes >= MAX_TOTAL_BYTES {
                    byte_limit = true;
                }
                match table.acquire_source(source, request.base.as_deref(), host) {
                    Ok(bytes) => match validate(bytes) {
                        Ok(validated) => {
                            let ValidatedFace {
                                bytes,
                                units_per_em,
                                ascent,
                                descent,
                                y_min,
                                y_max,
                            } = validated;
                            table.accepted_bytes = table.accepted_bytes.saturating_add(bytes.len());
                            table.entries.push(FaceEntry {
                                id: FontId(table.entries.len()),
                                family: request.rule.family.clone(),
                                weight: request.rule.weight,
                                style: request.rule.style,
                                bytes: bytes.into(),
                                units_per_em,
                                ascent,
                                descent,
                                y_min,
                                y_max,
                            });
                            accepted = true;
                            break;
                        }
                        Err(reason) => refused.record(format!(
                            "{} in {} from {}: {reason}",
                            request.rule.family,
                            request.origin,
                            source_name(source)
                        )),
                    },
                    Err(reason) => {
                        byte_limit |= reason.aggregate_limit;
                        refused.record(format!(
                            "{} in {} from {}: {reason}",
                            request.rule.family,
                            request.origin,
                            source_name(source)
                        ));
                    }
                }
            }
            if !accepted && table.attempts >= MAX_ATTEMPTS {
                break;
            }
        }
        if refused.count > 0 {
            warnings.push(crate::contracts::Warning::AuthoredFontsRefused {
                count: refused.count,
                examples: refused.examples,
            });
        }
        for (reached, kind, maximum) in [
            (face_limit, "accepted-face", MAX_FACES),
            (attempt_limit, "source-attempt", MAX_ATTEMPTS),
            (byte_limit, "accepted-byte", MAX_TOTAL_BYTES),
        ] {
            if reached {
                warnings
                    .push(crate::contracts::Warning::AuthoredFontResourceLimit { kind, maximum });
            }
        }
        debug_assert!(table.invariants_hold());
        table
    }

    fn acquire_source(
        &self,
        source: &crate::css::FaceSource,
        base: Option<&std::path::Path>,
        host: &dyn crate::contracts::RenderHost,
    ) -> Result<Vec<u8>, SourceRefusal> {
        let remaining = MAX_TOTAL_BYTES.saturating_sub(self.accepted_bytes);
        let room = MAX_FACE_BYTES.min(remaining);
        match source {
            crate::css::FaceSource::Local(_) => Err(SourceRefusal::other(
                "local() is refused; ambient font discovery is nondeterministic",
            )),
            crate::css::FaceSource::Url { value, format } => {
                if let Some(format) = format
                    && !matches!(format.as_str(), "truetype" | "ttf")
                {
                    return Err(SourceRefusal::other(format!(
                        "{format} is unsupported; only static TrueType is admitted"
                    )));
                }
                if value
                    .get(..5)
                    .is_some_and(|head| head.eq_ignore_ascii_case("data:"))
                {
                    return decode_data(value, room).map_err(|detail| SourceRefusal {
                        aggregate_limit: remaining < MAX_FACE_BYTES
                            && detail.contains("exceeds the"),
                        detail,
                    });
                }
                if value.contains("://") || value.starts_with("file:") {
                    return Err(SourceRefusal::other(
                        "network and file URLs are not fetched",
                    ));
                }
                let base = base.ok_or_else(|| {
                    SourceRefusal::other(
                        "no base exists for this relative font path; provide an explicit asset root",
                    )
                })?;
                let path = crate::core::confine(base, value).ok_or_else(|| {
                    SourceRefusal::other("the font path leaves its stylesheet base")
                })?;
                host.read_file(&path, room, crate::contracts::Requester::Document)
                    .map_err(|error| SourceRefusal {
                        aggregate_limit: remaining < MAX_FACE_BYTES
                            && matches!(error, crate::contracts::HostError::TooLarge { .. }),
                        detail: error.to_string(),
                    })
            }
        }
    }

    fn invariants_hold(&self) -> bool {
        self.entries.len() <= MAX_FACES
            && self.accepted_bytes <= MAX_TOTAL_BYTES
            && self.entries.iter().all(|entry| {
                !entry.family.is_empty()
                    && entry.bytes.len() <= MAX_FACE_BYTES
                    && entry.units_per_em > 0
                    && matches!(
                        entry.weight,
                        crate::contracts::FontWeight::Normal | crate::contracts::FontWeight::Bold
                    )
                    && matches!(
                        entry.style,
                        crate::contracts::FontStyle::Normal | crate::contracts::FontStyle::Italic
                    )
            })
    }

    pub(crate) fn select(
        &self,
        families: &[String],
        weight: crate::contracts::FontWeight,
        style: crate::contracts::FontStyle,
    ) -> Option<AuthoredFace> {
        self.select_matching(families, weight, style, |_| true)
    }

    pub(crate) fn select_for_text(
        &self,
        families: &[String],
        weight: crate::contracts::FontWeight,
        style: crate::contracts::FontStyle,
        text: &str,
    ) -> Option<AuthoredFace> {
        self.select_matching(families, weight, style, |entry| entry.covers(text))
    }

    fn select_matching(
        &self,
        families: &[String],
        weight: crate::contracts::FontWeight,
        style: crate::contracts::FontStyle,
        accepts: impl Fn(&FaceEntry) -> bool,
    ) -> Option<AuthoredFace> {
        families.iter().find_map(|family| {
            self.entries
                .iter()
                .find(|entry| {
                    entry.family.eq_ignore_ascii_case(family)
                        && entry.weight == weight
                        && entry.style == style
                        && accepts(entry)
                })
                .map(|entry| AuthoredFace {
                    id: entry.id,
                    bytes: entry.bytes.clone(),
                    units_per_em: entry.units_per_em,
                    ascent: entry.ascent,
                    descent: entry.descent,
                    y_min: entry.y_min,
                    y_max: entry.y_max,
                    shaping: self.shaping.clone(),
                })
        })
    }

    #[allow(dead_code)]
    pub(crate) fn entry(&self, id: FontId) -> Option<&FaceEntry> {
        self.entries.get(id.0)
    }

    pub(crate) fn shaping_warnings(&self) -> Vec<crate::contracts::Warning> {
        let Ok(state) = self.shaping.lock() else {
            return Vec::new();
        };
        let mut warnings = Vec::new();
        if state.refused > 0 {
            warnings.push(crate::contracts::Warning::AuthoredTextRefused {
                count: state.refused,
                reason: "the document shaping budget was exhausted",
                first: state.first_refused.clone().unwrap_or_default(),
            });
        }
        if state.context_splits > 0 {
            warnings.push(crate::contracts::Warning::AuthoredShapingContextSplit {
                count: state.context_splits,
                first: state.first_split.clone().unwrap_or_default(),
            });
        }
        if state.oversized_graphemes > 0 {
            warnings.push(crate::contracts::Warning::AuthoredTextRefused {
                count: state.oversized_graphemes,
                reason: "one grapheme exceeded the 256-scalar shaping ceiling",
                first: state.first_oversized_grapheme.clone().unwrap_or_default(),
            });
        }
        warnings
    }
}

impl FaceEntry {
    fn covers(&self, text: &str) -> bool {
        let Ok(font) = harfrust::FontRef::new(&self.bytes) else {
            return false;
        };
        let Ok(cmap) = font.cmap() else {
            return false;
        };
        text.chars().all(|character| {
            cmap.map_codepoint(character)
                .is_some_and(|glyph| glyph.to_u32() != 0)
        })
    }
}

impl AuthoredFace {
    pub(crate) fn shape(&self, text: &str, size: f32) -> Option<PositionedRun> {
        if text.chars().count() <= 256 {
            return self.shape_part(text, size);
        }
        let mut chunks = Vec::new();
        let mut chunk = String::new();
        let mut scalars = 0_usize;
        for grapheme in text.graphemes(true) {
            let count = grapheme.chars().count();
            if count > 256 {
                if let Ok(mut state) = self.shaping.lock() {
                    state.oversized_graphemes = state.oversized_graphemes.saturating_add(1);
                    if state.first_oversized_grapheme.is_none() {
                        state.first_oversized_grapheme = Some(grapheme.chars().take(80).collect());
                    }
                }
                return None;
            }
            if scalars.saturating_add(count) > 256 {
                chunks.push(std::mem::take(&mut chunk));
                scalars = 0;
            }
            chunk.push_str(grapheme);
            scalars = scalars.saturating_add(count);
        }
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        if let Ok(mut state) = self.shaping.lock() {
            state.context_splits = state.context_splits.saturating_add(1);
            if state.first_split.is_none() {
                state.first_split = Some(text.chars().take(80).collect());
            }
        }
        let mut chunks = chunks.into_iter();
        let mut positioned = self.shape_part(&chunks.next()?, size)?;
        for chunk in chunks {
            positioned.append(self.shape_part(&chunk, size)?);
        }
        Some(positioned)
    }

    fn shape_part(&self, text: &str, size: f32) -> Option<PositionedRun> {
        let key = (self.id.0, size.to_bits(), text.to_owned());
        {
            let mut state = self.shaping.lock().ok()?;
            if let Some(positioned) = state.cache.get(&key) {
                return Some(positioned.clone());
            }
            if state.refused_keys.contains(&key) {
                return None;
            }
            let scalars = text.chars().count();
            let operations = u64::try_from(scalars)
                .unwrap_or(u64::MAX)
                .saturating_mul(4_096)
                .max(65_536);
            let capacity = u64::try_from(scalars)
                .unwrap_or(u64::MAX)
                .saturating_mul(256)
                .max(65_536);
            let admitted = state.calls < MAX_SHAPING_CALLS
                && state.scalars.saturating_add(scalars) <= MAX_SHAPING_SCALARS
                && state.operations.saturating_add(operations) <= MAX_SHAPING_OPERATIONS
                && state.capacity_units.saturating_add(capacity) <= MAX_SHAPING_CAPACITY_UNITS;
            if !admitted {
                state.refused_keys.insert(key.clone());
                state.record_refusal(text);
                return None;
            }
            state.calls = state.calls.saturating_add(1);
            state.scalars = state.scalars.saturating_add(scalars);
            state.operations = state.operations.saturating_add(operations);
            state.capacity_units = state.capacity_units.saturating_add(capacity);
        }
        let font = harfrust::FontRef::new(&self.bytes).ok()?;
        let data = harfrust::ShaperData::new(&font);
        let shaper = data.shaper(&font).build();
        let mut input = harfrust::UnicodeBuffer::new();
        input.push_str(text);
        input.guess_segment_properties();
        let glyph_buffer = shaper.shape(input, harfrust::ShapeOptions::new());
        let scale = size / f32::from(self.units_per_em);
        let mut advance = 0.0_f32;
        let glyphs = glyph_buffer
            .glyph_infos()
            .iter()
            .zip(glyph_buffer.glyph_positions())
            .map(|(info, position)| {
                let id = u16::try_from(info.glyph_id).ok()?;
                let x_advance = font_unit(position.x_advance) * scale;
                advance += x_advance;
                Some(PositionedGlyph {
                    id,
                    cluster: info.cluster,
                    x_advance,
                    x_offset: font_unit(position.x_offset) * scale,
                    y_offset: font_unit(position.y_offset) * scale,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let positioned = PositionedRun {
            face: self.id,
            source: text.to_owned(),
            glyphs,
            advance,
            ink_above: self.ink_extent(size).0,
            ink_below: self.ink_extent(size).1,
        };
        let mut state = self.shaping.lock().ok()?;
        if state
            .retained_glyphs
            .saturating_add(positioned.glyphs.len())
            > MAX_RETAINED_GLYPHS
        {
            state.refused_keys.insert(key);
            state.record_refusal(text);
            return None;
        }
        state.retained_glyphs = state
            .retained_glyphs
            .saturating_add(positioned.glyphs.len());
        state.cache.insert(key, positioned.clone());
        Some(positioned)
    }

    pub(crate) fn baseline_offset(&self, size: f32, line_height: f32) -> f32 {
        let scale = size / f32::from(self.units_per_em);
        let ascent = f32::from(self.ascent) * scale;
        let descent = f32::from(self.descent).abs() * scale;
        ascent + (line_height - ascent - descent).max(0.0) / 2.0
    }

    pub(crate) fn ink_extent(&self, size: f32) -> (f32, f32) {
        let scale = size / f32::from(self.units_per_em);
        (
            f32::from(self.y_max).max(0.0) * scale,
            f32::from(self.y_min).min(0.0).abs() * scale,
        )
    }
}

impl ShapingState {
    fn record_refusal(&mut self, text: &str) {
        self.refused = self.refused.saturating_add(1);
        if self.first_refused.is_none() {
            self.first_refused = Some(text.chars().take(80).collect());
        }
    }
}

pub(crate) fn text_admission(text: &str) -> Result<(), &'static str> {
    if text.chars().any(|character| {
        matches!(
            character,
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
    }) {
        return Err("it contains an explicit bidi-formatting control");
    }
    let bidi = unicode_bidi::BidiInfo::new(text, None);
    if bidi.levels.iter().any(|level| level.number() != 0) {
        return Err("right-to-left or mixed-direction shaping is outside this bounded slice");
    }
    if text.chars().any(|character| {
        !matches!(
            character.script(),
            unicode_script::Script::Latin
                | unicode_script::Script::Greek
                | unicode_script::Script::Cyrillic
                | unicode_script::Script::Common
                | unicode_script::Script::Inherited
        )
    }) {
        return Err("its script is outside Latin, Greek, Cyrillic, Common and Inherited");
    }
    Ok(())
}

fn source_name(source: &crate::css::FaceSource) -> String {
    match source {
        crate::css::FaceSource::Url { value, .. } => value.clone(),
        crate::css::FaceSource::Local(value) => format!("local({value})"),
    }
}

fn validate(bytes: Vec<u8>) -> Result<ValidatedFace, &'static str> {
    match bytes.get(..4) {
        Some(b"ttcf") => return Err("font collections are unsupported"),
        Some(b"OTTO") => return Err("CFF/OpenType outlines are unsupported"),
        Some(b"wOFF" | b"wOF2") => return Err("WOFF and WOFF2 are unsupported"),
        _ => {}
    }
    let font = harfrust::FontRef::new(&bytes).map_err(|_| "malformed static TrueType font")?;
    let head = font.head().map_err(|_| "malformed or missing head table")?;
    let units_per_em = head.units_per_em();
    if units_per_em == 0 {
        return Err("invalid zero units-per-em");
    }
    let hhea = font.hhea().map_err(|_| "malformed or missing hhea table")?;
    if hhea.number_of_h_metrics() == 0 {
        return Err("invalid zero horizontal metrics");
    }
    let maxp = font.maxp().map_err(|_| "malformed or missing maxp table")?;
    if maxp.num_glyphs() == 0 {
        return Err("invalid zero glyph count");
    }
    font.hmtx().map_err(|_| "malformed or missing hmtx table")?;
    font.cmap()
        .map_err(|_| "malformed or missing Unicode cmap")?;
    let glyf = font.glyf().map_err(|_| "missing glyf TrueType outlines")?;
    let loca = font
        .loca(None)
        .map_err(|_| "malformed or missing loca table")?;
    if loca.len() != usize::from(maxp.num_glyphs())
        || !loca.all_offsets_are_ascending()
        || loca.get_raw(loca.len()).is_none_or(|last| {
            usize::try_from(last).map_or(true, |last| last > glyf.offset_data().len())
        })
    {
        return Err("glyph count and loca/glyf bounds disagree");
    }
    if font.fvar().is_ok() {
        return Err("variable fonts are unsupported");
    }
    if font.cff().is_ok() || font.cff2().is_ok() {
        return Err("CFF outlines are unsupported");
    }
    if font.colr().is_ok() || font.cbdt().is_ok() || font.sbix().is_ok() || font.svg().is_ok() {
        return Err("colour, bitmap and SVG glyph paths are unsupported");
    }
    let (ascent, descent) = font.os2().map_or_else(
        |_| (hhea.ascender().to_i16(), hhea.descender().to_i16()),
        |os2| {
            if os2
                .fs_selection()
                .contains(read_fonts::tables::os2::SelectionFlags::USE_TYPO_METRICS)
            {
                (os2.s_typo_ascender(), os2.s_typo_descender())
            } else {
                (hhea.ascender().to_i16(), hhea.descender().to_i16())
            }
        },
    );
    if ascent <= 0 || descent >= 0 {
        return Err("unusable vertical font metrics");
    }
    let y_min = head.y_min();
    let y_max = head.y_max();
    if y_max <= 0 || y_min >= 0 {
        return Err("unusable global font bounds");
    }
    Ok(ValidatedFace {
        bytes,
        units_per_em,
        ascent,
        descent,
        y_min,
        y_max,
    })
}

#[allow(clippy::as_conversions, clippy::cast_precision_loss)]
fn font_unit(value: i32) -> f32 {
    value as f32
}

fn decode_data(value: &str, limit: usize) -> Result<Vec<u8>, String> {
    let Some((meta, payload)) = value.get(5..).and_then(|rest| rest.split_once(',')) else {
        return Err("malformed data font URI".to_owned());
    };
    if !matches!(
        meta.to_ascii_lowercase().as_str(),
        "font/ttf;base64" | "application/x-font-ttf;base64"
    ) {
        return Err("data font must declare font/ttf and base64".to_owned());
    }
    let encoded_limit = limit.saturating_add(2) / 3 * 4;
    if payload.len() > encoded_limit {
        return Err(format!("font exceeds the {limit}-byte limit"));
    }
    decode_base64(payload, limit)
}

fn decode_base64(text: &str, limit: usize) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(4) {
        return Err("malformed base64 in data font".to_owned());
    }
    let value = |byte: u8| -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some(u32::from(byte - b'A')),
            b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity((text.len() / 4 * 3).min(limit));
    let groups = text.as_bytes().chunks(4);
    let count = groups.len();
    for (ordinal, group) in groups.enumerate() {
        if ordinal.saturating_add(1) < count && group.contains(&b'=') {
            return Err("malformed base64 padding in data font".to_owned());
        }
        let a = group
            .first()
            .copied()
            .and_then(value)
            .ok_or_else(|| "malformed base64 in data font".to_owned())?;
        let b = group
            .get(1)
            .copied()
            .and_then(value)
            .ok_or_else(|| "malformed base64 in data font".to_owned())?;
        let c = group.get(2).copied().unwrap_or(b'=');
        let d = group.get(3).copied().unwrap_or(b'=');
        let mut packed = (a << 18) | (b << 12);
        let mut produced = 1;
        if c != b'=' {
            packed |= value(c).ok_or_else(|| "malformed base64 in data font".to_owned())? << 6;
            produced = 2;
        }
        if d != b'=' {
            if c == b'=' {
                return Err("malformed base64 padding in data font".to_owned());
            }
            packed |= value(d).ok_or_else(|| "malformed base64 in data font".to_owned())?;
            produced = 3;
        }
        if out.len().saturating_add(produced) > limit {
            return Err(format!("font exceeds the {limit}-byte limit"));
        }
        let octet = |shift: u32| u8::try_from((packed >> shift) & 0xff).unwrap_or_default();
        let bytes = [octet(16), octet(8), octet(0)];
        out.extend_from_slice(bytes.get(..produced).unwrap_or_default());
    }
    Ok(out)
}

struct SourceRefusal {
    detail: String,
    aggregate_limit: bool,
}

impl SourceRefusal {
    fn other(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            aggregate_limit: false,
        }
    }
}

impl std::fmt::Display for SourceRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

#[derive(Default)]
struct Refusals {
    count: usize,
    examples: Vec<String>,
}

impl Refusals {
    fn record(&mut self, example: String) {
        self.count = self.count.saturating_add(1);
        if self.examples.len() < 3 {
            self.examples.push(example);
        }
    }
}
