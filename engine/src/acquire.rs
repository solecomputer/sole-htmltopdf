use crate::contracts::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Inline { media: Media, bytes: Vec<u8> },

    Relative(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Media {
    Png,
    Jpeg,
}

pub(crate) const MAX_INLINE_ENCODED_BYTES: usize = 6 * 1024 * 1024;

pub(crate) fn parse_source(src: &str) -> Result<Source, Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    let trimmed = src.trim();

    if trimmed.is_empty() {
        return Err(refuse("an image with no source"));
    }

    let scheme_lowered = trimmed
        .split_once(':')
        .map(|(scheme, rest)| (scheme.to_ascii_lowercase(), rest));
    if let Some((scheme, rest)) = scheme_lowered.as_ref()
        && scheme == "data"
    {
        return parse_data_uri(rest);
    }

    if trimmed.contains("://")
        || scheme_lowered
            .as_ref()
            .is_some_and(|(scheme, _)| scheme == "file")
    {
        return Err(refuse(
            "an image named by a URL: this version reads local files and inline data only",
        ));
    }
    if trimmed.starts_with('#') {
        return Err(refuse("an image source that is only a fragment"));
    }
    Ok(Source::Relative(trimmed.to_owned()))
}

fn parse_data_uri(rest: &str) -> Result<Source, Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    let (meta, payload) = rest
        .split_once(',')
        .ok_or_else(|| refuse("an inline image with no comma between its type and its data"))?;

    let mut parts = meta.split(';');
    let media = match parts.next().unwrap_or_default().trim() {
        "image/png" => Media::Png,
        "image/jpeg" | "image/jpg" => Media::Jpeg,
        "" => return Err(refuse("an inline image that declares no media type")),
        _ => {
            return Err(refuse(
                "an inline image whose media type is not PNG or JPEG",
            ));
        }
    };

    let mut base64 = false;
    for parameter in parts {
        if parameter.trim().eq_ignore_ascii_case("base64") {
            base64 = true;
        } else {
            return Err(refuse(
                "an inline image carrying a parameter this version does not accept",
            ));
        }
    }
    if !base64 {
        return Err(refuse(
            "an inline image that is not base64: percent-encoded payloads are not decoded",
        ));
    }
    if payload.len() > MAX_INLINE_ENCODED_BYTES {
        return Err(refuse("an inline image larger than the inline ceiling"));
    }
    let bytes = decode_base64(payload)?;

    let signature_is_png = bytes.get(..4) == Some(&[0x89, b'P', b'N', b'G'][..]);
    let signature_is_jpeg = bytes.get(..2) == Some(&[0xFF, 0xD8][..]);
    let agrees = match media {
        Media::Png => signature_is_png,
        Media::Jpeg => signature_is_jpeg,
    };
    if !agrees {
        return Err(refuse(
            "an inline image whose declared type disagrees with its own contents",
        ));
    }
    Ok(Source::Inline { media, bytes })
}

fn decode_base64(text: &str) -> Result<Vec<u8>, Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(refuse(
            "an inline image whose base64 is not a whole number of groups",
        ));
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
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let groups = bytes.len() / 4;
    for (ordinal, group) in bytes.chunks(4).enumerate() {
        let last = ordinal + 1 == groups;
        if !last && group.contains(&b'=') {
            return Err(refuse(
                "an inline image whose base64 pads before its final group",
            ));
        }
        let (a, b) = (
            group.first().copied().unwrap_or(b'='),
            group.get(1).copied().unwrap_or(b'='),
        );
        let (c, d) = (
            group.get(2).copied().unwrap_or(b'='),
            group.get(3).copied().unwrap_or(b'='),
        );
        let (Some(first), Some(second)) = (value(a), value(b)) else {
            return Err(refuse(
                "an inline image whose base64 holds a character outside the alphabet",
            ));
        };
        let mut packed = (first << 18) | (second << 12);
        let mut produced = 1;
        if c != b'=' {
            let Some(third) = value(c) else {
                return Err(refuse(
                    "an inline image whose base64 holds a character outside the alphabet",
                ));
            };
            packed |= third << 6;
            produced = 2;
        }
        if d != b'=' {
            if c == b'=' {
                return Err(refuse(
                    "an inline image whose base64 pads in the middle of a group",
                ));
            }
            let Some(fourth) = value(d) else {
                return Err(refuse(
                    "an inline image whose base64 holds a character outside the alphabet",
                ));
            };
            packed |= fourth;
            produced = 3;
        }

        let spare = match produced {
            1 => packed & 0xFFFF,
            2 => packed & 0xFF,
            _ => 0,
        };
        if spare != 0 {
            return Err(refuse(
                "an inline image whose base64 sets bits its padding says are unused",
            ));
        }
        let narrow = |shifted: u32| -> u8 { u8::try_from(shifted & 0xFF).unwrap_or_default() };
        let octets = [narrow(packed >> 16), narrow(packed >> 8), narrow(packed)];
        out.extend_from_slice(octets.get(..produced).unwrap_or_default());
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AssetId(usize);

impl AssetId {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Assets {
    pub(crate) fonts: crate::font::FontTable,
    entries: Vec<crate::image::Embeddable>,

    by_source: Vec<(String, AssetId)>,

    charged: usize,

    refused: Vec<(String, Skipped)>,

    profiles_ignored: usize,

    work: usize,
}

pub(crate) const MAX_IMAGES: usize = 20;

pub(crate) const MAX_IMAGE_ATTEMPTS: usize = MAX_IMAGES * 4;

pub(crate) const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) const MAX_TOTAL_IMAGE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skipped {
    Denied,

    OutsideRoot,

    NoBase,
    NotFound,
    TooLarge,
    Unreadable,

    Disabled,

    EscapesDirectory,

    OverBudget,

    Refused(&'static str),
}

impl Assets {
    pub(crate) fn set_fonts(&mut self, fonts: crate::font::FontTable) {
        self.fonts = fonts;
    }

    #[must_use]
    pub(crate) const fn profiles_ignored(&self) -> usize {
        self.profiles_ignored
    }

    #[must_use]
    pub(crate) fn get(&self, id: AssetId) -> Option<&crate::image::Embeddable> {
        self.entries.get(id.0)
    }

    pub(crate) fn resolve(
        &mut self,
        src: &str,
        host: &dyn crate::contracts::RenderHost,
        base: Option<&std::path::Path>,
    ) -> Result<AssetId, Skipped> {
        let outcome = self.resolve_once(src, host, base);
        if let Err(cause) = outcome {
            self.refused.push((src.to_owned(), cause));
        }
        outcome
    }

    fn resolve_once(
        &mut self,
        src: &str,
        host: &dyn crate::contracts::RenderHost,
        base: Option<&std::path::Path>,
    ) -> Result<AssetId, Skipped> {
        if let Some((_, id)) = self.by_source.iter().find(|(seen, _)| seen == src) {
            return Ok(*id);
        }

        if let Some((_, cause)) = self.refused.iter().find(|(seen, _)| seen == src) {
            return Err(*cause);
        }

        self.work = self.work.saturating_add(1);

        if self.work > MAX_IMAGE_ATTEMPTS {
            return Err(Skipped::OverBudget);
        }

        if self.entries.len() >= MAX_IMAGES || self.charged >= MAX_TOTAL_IMAGE_BYTES {
            return Err(Skipped::OverBudget);
        }
        let source = parse_source(src).map_err(|error| refusal_of(&error))?;
        let bytes = match source {
            Source::Inline { bytes, .. } => bytes,
            Source::Relative(path) => {
                let base = base.ok_or(Skipped::NoBase)?;

                let resolved =
                    crate::core::confine(base, &path).ok_or(Skipped::EscapesDirectory)?;
                let room = MAX_IMAGE_BYTES.min(MAX_TOTAL_IMAGE_BYTES.saturating_sub(self.charged));
                match host.read_file(&resolved, room, crate::contracts::Requester::Document) {
                    Ok(bytes) => bytes,
                    Err(
                        crate::contracts::HostError::Denied { .. }
                        | crate::contracts::HostError::AccessDisabled { .. },
                    ) => return Err(Skipped::Denied),
                    Err(crate::contracts::HostError::OutsideRoot { .. }) => {
                        return Err(Skipped::OutsideRoot);
                    }
                    Err(crate::contracts::HostError::NotFound { .. }) => {
                        return Err(Skipped::NotFound);
                    }
                    Err(crate::contracts::HostError::TooLarge { .. }) => {
                        self.charged = self.charged.saturating_add(room);
                        return Err(Skipped::TooLarge);
                    }
                    Err(_) => return Err(Skipped::Unreadable),
                }
            }
        };
        if bytes.len() > MAX_IMAGE_BYTES
            || self.charged.saturating_add(bytes.len()) > MAX_TOTAL_IMAGE_BYTES
        {
            self.charged = self.charged.saturating_add(bytes.len());
            return Err(Skipped::OverBudget);
        }
        self.charged = self.charged.saturating_add(bytes.len());

        let embeddable = if bytes.get(..2) == Some(&[0xFF, 0xD8][..]) {
            crate::image::read_jpeg(&bytes)
        } else {
            crate::image::read_png(&bytes)
        }
        .map_err(|error| refusal_of(&error))?;

        Ok(self.insert(src.to_owned(), embeddable))
    }

    fn insert(&mut self, src: String, image: crate::image::Embeddable) -> AssetId {
        let existing = self.entries.iter().position(|held| {
            held.width() == image.width()
                && held.height() == image.height()
                && held.colour() == image.colour()
                && held.payload() == image.payload()
                && held.bytes() == image.bytes()
        });
        let id = if let Some(index) = existing {
            AssetId(index)
        } else {
            if image.profile_ignored() {
                self.profiles_ignored = self.profiles_ignored.saturating_add(1);
            }
            self.entries.push(image);
            AssetId(self.entries.len().saturating_sub(1))
        };
        self.by_source.push((src, id));
        id
    }
}

#[allow(
    missing_debug_implementations,
    reason = "holds a trait object; there is nothing to print but the geometry"
)]
#[derive(Clone, Copy)]
pub(crate) struct World<'a> {
    pub(crate) host: &'a dyn crate::contracts::RenderHost,
    pub(crate) base: Option<&'a std::path::Path>,
    pub(crate) images: crate::contracts::Images,

    pub(crate) container: f32,

    pub(crate) usable: f32,
}

pub(crate) const POINTS_PER_PIXEL: f32 = 0.75;

pub(crate) fn resolve_tree(
    node: &mut crate::contracts::BoxNode,
    world: &World<'_>,
    assets: &mut Assets,
    skipped: &mut Vec<Skipped>,
) {
    if let crate::contracts::InlineContent::Replaced(request) = &node.content {
        let request = request.clone();

        if world.images == crate::contracts::Images::Off {
            skipped.push(Skipped::Disabled);
            node.content = fallback_for(request.alt);
            for child in &mut node.children {
                resolve_tree(child, world, assets, skipped);
            }
            return;
        }
        match assets.resolve(&request.source, world.host, world.base) {
            Ok(id) => {
                let image = assets.get(id).map(|image| (image.width(), image.height()));
                if let Some((pixels_wide, pixels_high)) = image {
                    let used = used_size(&request, pixels_wide, pixels_high, world);
                    node.content =
                        crate::contracts::InlineContent::Placed(crate::contracts::PlacedImage {
                            asset: id.0,
                            width: used.0,
                            height: used.1,
                            acquired_scale: used.2,
                        });
                }
            }
            Err(cause) => {
                skipped.push(cause);
                node.content = fallback_for(request.alt);
            }
        }
    }
    for child in &mut node.children {
        resolve_tree(child, world, assets, skipped);
    }
}

fn refusal_of(error: &Error) -> Skipped {
    if let Error::ImageRefused { what } = error {
        return Skipped::Refused(what);
    }

    debug_assert!(
        false,
        "an image reader returned a non-refusal error: {error:?}"
    );
    Skipped::Refused("the image reader failed in a way it does not have a name for")
}

fn fallback_for(alt: Option<String>) -> crate::contracts::InlineContent {
    match alt.filter(|alt| !alt.is_empty()) {
        Some(alt) => crate::contracts::InlineContent::Text(alt),
        None => crate::contracts::InlineContent::Empty,
    }
}

fn used_size(
    request: &crate::contracts::ImageRequest,
    pixels_wide: u16,
    pixels_high: u16,
    world: &World<'_>,
) -> (f32, f32, f32) {
    let intrinsic_w = pixels_from(pixels_wide);
    let intrinsic_h = pixels_from(pixels_high);
    let (mut width, mut height) = match (request.width, request.height) {
        (Some(w), Some(h)) => (w * POINTS_PER_PIXEL, h * POINTS_PER_PIXEL),
        (Some(w), None) => {
            let w = w * POINTS_PER_PIXEL;
            (w, w * intrinsic_h / intrinsic_w.max(f32::EPSILON))
        }
        (None, Some(h)) => {
            let h = h * POINTS_PER_PIXEL;
            (h * intrinsic_w / intrinsic_h.max(f32::EPSILON), h)
        }
        (None, None) => (intrinsic_w, intrinsic_h),
    };
    let scale = 1.0_f32
        .min(world.container / width.max(f32::EPSILON))
        .min(world.usable / height.max(f32::EPSILON));
    if scale < 1.0 {
        width *= scale;
        height *= scale;
    }

    (width, height, scale)
}

fn pixels_from(pixels: u16) -> f32 {
    f32::from(pixels) * POINTS_PER_PIXEL
}
