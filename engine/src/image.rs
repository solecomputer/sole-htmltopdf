use crate::contracts::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColourSpace {
    Gray,

    Rgb,
}

impl ColourSpace {
    #[must_use]
    pub(crate) const fn components(self) -> u8 {
        match self {
            Self::Gray => 1,
            Self::Rgb => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Payload {
    PngIdat,

    JpegFile,
}

#[derive(Debug, Clone)]
pub(crate) struct Embeddable {
    width: u16,
    height: u16,
    colour: ColourSpace,
    payload: Payload,
    bytes: Vec<u8>,

    profile_ignored: bool,
}

pub(crate) const MAX_IMAGE_DIMENSION: u16 = 20_000;

pub(crate) const MAX_DECODED_SAMPLE_PRODUCT: u64 = 100_000_000;

impl Embeddable {
    #[must_use]
    pub(crate) const fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub(crate) const fn height(&self) -> u16 {
        self.height
    }

    #[must_use]
    pub(crate) const fn colour(&self) -> ColourSpace {
        self.colour
    }

    #[must_use]
    pub(crate) const fn payload(&self) -> Payload {
        self.payload
    }

    #[must_use]
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn admit(
        width: u32,
        height: u32,
        colour: ColourSpace,
        payload: Payload,
        bytes: Vec<u8>,
        profile_ignored: bool,
    ) -> Result<Self, Error> {
        let (width, height) = admit_geometry(width, height, colour)?;
        Ok(Self {
            width,
            height,
            colour,
            payload,
            bytes,
            profile_ignored,
        })
    }

    #[must_use]
    pub(crate) const fn profile_ignored(&self) -> bool {
        self.profile_ignored
    }
}

fn admit_geometry(width: u32, height: u32, colour: ColourSpace) -> Result<(u16, u16), Error> {
    {
        if width == 0 || height == 0 {
            return Err(Error::ImageRefused {
                what: "an image declares a zero dimension",
            });
        }

        let admit = |value: u32| {
            u16::try_from(value)
                .ok()
                .filter(|small| *small <= MAX_IMAGE_DIMENSION)
                .ok_or(Error::ImageRefused {
                    what: "an image declares a dimension above the ceiling",
                })
        };
        let (width_small, height_small) = (admit(width)?, admit(height)?);
        let samples = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|area| area.checked_mul(u64::from(colour.components())));
        match samples {
            Some(total) if total <= MAX_DECODED_SAMPLE_PRODUCT => {}
            _ => {
                return Err(Error::ImageRefused {
                    what: "an image declares more samples than the ceiling allows",
                });
            }
        }
        Ok((width_small, height_small))
    }
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

pub(crate) fn read_png(bytes: &[u8]) -> Result<Embeddable, Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    let PngChunks {
        width,
        height,
        colour,
        idat,
        profile,
    } = walk_png_chunks(bytes)?;

    admit_geometry(width, height, colour)?;

    let row = u64::from(width)
        .checked_mul(u64::from(colour.components()))
        .and_then(|row| row.checked_add(1))
        .ok_or_else(|| refuse("a PNG's declared size overflows its own row arithmetic"))?;
    let expected = row
        .checked_mul(u64::from(height))
        .ok_or_else(|| refuse("a PNG's declared size overflows its own row arithmetic"))?;
    if !inflates_to_exactly(&idat, expected, row) {
        return Err(refuse(
            "a PNG whose compressed data does not decode to the size its header declares, \
             or whose rows do not carry the filter bytes a predictor needs",
        ));
    }

    Embeddable::admit(width, height, colour, Payload::PngIdat, idat, profile)
}

struct PngChunks {
    width: u32,
    height: u32,
    colour: ColourSpace,
    idat: Vec<u8>,
    profile: bool,
}

fn walk_png_chunks(bytes: &[u8]) -> Result<PngChunks, Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    if bytes.get(..PNG_SIGNATURE.len()) != Some(&PNG_SIGNATURE[..]) {
        return Err(refuse("a PNG does not begin with the PNG signature"));
    }

    let mut at = PNG_SIGNATURE.len();
    let mut header: Option<(u32, u32, ColourSpace)> = None;
    let mut idat: Vec<u8> = Vec::new();
    let mut ended = false;
    let mut idat_closed = false;
    let mut profile = false;

    while at < bytes.len() {
        let declared = u32::from_be_bytes(
            bytes
                .get(at..at + 4)
                .and_then(|slice| slice.try_into().ok())
                .ok_or_else(|| refuse("a PNG chunk header runs past the end of the file"))?,
        );
        let length = usize::try_from(declared)
            .map_err(|_| refuse("a PNG chunk length does not fit this machine's addressing"))?;
        let tag: [u8; 4] = bytes
            .get(at + 4..at + 8)
            .and_then(|slice| slice.try_into().ok())
            .ok_or_else(|| refuse("a PNG chunk header runs past the end of the file"))?;
        let data = bytes
            .get(at + 8..at + 8 + length)
            .ok_or_else(|| refuse("a PNG chunk claims more data than the file holds"))?;
        let stated = u32::from_be_bytes(
            bytes
                .get(at + 8 + length..at + 12 + length)
                .and_then(|slice| slice.try_into().ok())
                .ok_or_else(|| refuse("a PNG chunk is missing its checksum"))?,
        );
        if crc32(tag, data) != stated {
            return Err(refuse("a PNG chunk fails its own checksum"));
        }
        at += 12 + length;

        match &tag {
            b"IHDR" => {
                if header.is_some() {
                    return Err(refuse("a PNG carries more than one header chunk"));
                }
                header = Some(read_ihdr(data)?);
            }
            b"IDAT" => {
                if header.is_none() {
                    return Err(refuse("a PNG puts image data before its header"));
                }
                if idat_closed {
                    return Err(refuse("a PNG splits its image data around another chunk"));
                }
                idat.extend_from_slice(data);
            }
            b"IEND" => {
                ended = true;
                break;
            }
            b"tRNS" => {
                return Err(refuse(
                    "a PNG with transparency: it needs a soft mask this version does not build",
                ));
            }
            b"iCCP" => {
                profile = true;
            }
            other => {
                if !idat.is_empty() {
                    idat_closed = true;
                }

                if other[0].is_ascii_uppercase() {
                    return Err(refuse(
                        "a PNG carries a critical chunk this version cannot read",
                    ));
                }
            }
        }
    }

    if !ended {
        return Err(refuse("a PNG has no end marker"));
    }
    let (width, height, colour) = header.ok_or_else(|| refuse("a PNG has no header chunk"))?;
    if idat.is_empty() {
        return Err(refuse("a PNG carries no image data"));
    }

    Ok(PngChunks {
        width,
        height,
        colour,
        idat,
        profile,
    })
}

pub(crate) fn read_jpeg(bytes: &[u8]) -> Result<Embeddable, Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    if bytes.get(..2) != Some(&[0xFF, 0xD8][..]) {
        return Err(refuse(
            "a JPEG that does not begin with a start-of-image marker",
        ));
    }

    let mut at = 2_usize;
    let mut frame: Option<Frame> = None;
    let mut adobe_transform: Option<u8> = None;
    let mut ended = false;
    let mut scanned = false;

    while let Some(marker) = next_marker(bytes, &mut at)? {
        match marker {
            0xC0 | 0xC1 => {
                if frame.is_some() {
                    return Err(refuse("a JPEG carries more than one frame header"));
                }
                frame = Some(read_sof(segment(bytes, &mut at)?)?);
            }

            0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF => {
                return Err(refuse(
                    "a JPEG that is not baseline or extended sequential: progressive, \
                     lossless, hierarchical and arithmetic coding are not embedded",
                ));
            }

            0xEE => {
                let data = segment(bytes, &mut at)?;
                if data.get(..5) == Some(b"Adobe") {
                    adobe_transform = data.last().copied();
                }
            }

            0xDA => {
                let _ = segment(bytes, &mut at)?;
                scanned = true;
                skip_entropy(bytes, &mut at);
            }
            0xD9 => {
                ended = true;
                break;
            }
            _ => {
                let _ = segment(bytes, &mut at)?;
            }
        }
    }

    if !ended {
        return Err(refuse("a JPEG with no end-of-image marker"));
    }

    if !scanned {
        return Err(refuse(
            "a JPEG with no scan: its frame header describes nothing",
        ));
    }
    let frame = frame.ok_or_else(|| refuse("a JPEG with no frame header"))?;
    let (width, height, colour) = (frame.width, frame.height, frame.colour);

    if colour == ColourSpace::Rgb {
        if frame.labelled_rgb && adobe_transform.is_none() {
            return Err(refuse(
                "a JPEG whose components are labelled RGB rather than YCbCr: its channels \
                 would be drawn in the wrong order",
            ));
        }
        match adobe_transform {
            None | Some(1) => {}
            Some(_) => {
                return Err(refuse(
                    "a JPEG whose Adobe colour transform is not YCbCr: its channels would \
                     be drawn in the wrong order",
                ));
            }
        }
    }

    Embeddable::admit(
        width,
        height,
        colour,
        Payload::JpegFile,
        bytes.to_vec(),
        false,
    )
}

fn next_marker(bytes: &[u8], at: &mut usize) -> Result<Option<u8>, Error> {
    while let Some(byte) = bytes.get(*at) {
        if *byte != 0xFF {
            return Err(Error::ImageRefused {
                what: "a JPEG segment does not start where the previous one ended",
            });
        }

        let mut code = *at + 1;
        while bytes.get(code) == Some(&0xFF) {
            code += 1;
        }
        let Some(found) = bytes.get(code).copied() else {
            return Ok(None);
        };
        *at = code + 1;

        if (0xD0..=0xD7).contains(&found) || found == 0x01 {
            continue;
        }
        return Ok(Some(found));
    }
    Ok(None)
}

fn segment<'a>(bytes: &'a [u8], at: &mut usize) -> Result<&'a [u8], Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    let length = usize::from(u16::from_be_bytes(
        bytes
            .get(*at..*at + 2)
            .and_then(|slice| slice.try_into().ok())
            .ok_or_else(|| refuse("a JPEG segment header runs past the end of the file"))?,
    ));

    let payload = length
        .checked_sub(2)
        .ok_or_else(|| refuse("a JPEG segment declares a length below its own header"))?;
    let data = bytes
        .get(*at + 2..*at + 2 + payload)
        .ok_or_else(|| refuse("a JPEG segment claims more data than the file holds"))?;
    *at += 2 + payload;
    Ok(data)
}

fn skip_entropy(bytes: &[u8], at: &mut usize) {
    while let Some(byte) = bytes.get(*at) {
        if *byte != 0xFF {
            *at += 1;
            continue;
        }
        match bytes.get(*at + 1) {
            Some(0x00) => *at += 2,
            Some(next) if (0xD0..=0xD7).contains(next) => *at += 2,
            Some(_) => return,
            None => {
                *at += 1;
                return;
            }
        }
    }
}

struct Frame {
    width: u32,
    height: u32,
    colour: ColourSpace,

    labelled_rgb: bool,
}

fn read_sof(data: &[u8]) -> Result<Frame, Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    let byte = |at: usize| -> Result<u8, Error> {
        data.get(at)
            .copied()
            .ok_or_else(|| refuse("a JPEG frame header is too short"))
    };
    let field = |at: usize| -> Result<u32, Error> {
        Ok(u32::from(u16::from_be_bytes(
            data.get(at..at + 2)
                .and_then(|slice| slice.try_into().ok())
                .ok_or_else(|| refuse("a JPEG frame header is too short"))?,
        )))
    };
    if byte(0)? != 8 {
        return Err(refuse("a JPEG that is not 8 bits per component"));
    }
    let height = field(1)?;
    let width = field(3)?;
    let colour = match byte(5)? {
        1 => ColourSpace::Gray,
        3 => ColourSpace::Rgb,
        4 => {
            return Err(refuse(
                "a four-component JPEG: CMYK needs a colour transform this version does \
                 not apply",
            ));
        }
        _ => {
            return Err(refuse(
                "a JPEG with a component count this version cannot describe",
            ));
        }
    };

    let labelled_rgb = colour == ColourSpace::Rgb
        && [b'R', b'G', b'B']
            .iter()
            .enumerate()
            .all(|(index, wanted)| byte(6 + index * 3).is_ok_and(|found| found == *wanted));
    Ok(Frame {
        width,
        height,
        colour,
        labelled_rgb,
    })
}

fn read_ihdr(data: &[u8]) -> Result<(u32, u32, ColourSpace), Error> {
    let refuse = |what: &'static str| Error::ImageRefused { what };
    let field = |at: usize| -> Result<u32, Error> {
        Ok(u32::from_be_bytes(
            data.get(at..at + 4)
                .and_then(|slice| slice.try_into().ok())
                .ok_or_else(|| refuse("a PNG header chunk is the wrong size"))?,
        ))
    };
    if data.len() != 13 {
        return Err(refuse("a PNG header chunk is the wrong size"));
    }
    let width = field(0)?;
    let height = field(4)?;
    let byte = |at: usize| -> Result<u8, Error> {
        data.get(at)
            .copied()
            .ok_or_else(|| refuse("a PNG header chunk is the wrong size"))
    };
    if byte(12)? != 0 {
        return Err(refuse(
            "an interlaced PNG: Adam7 cannot be described by a PDF predictor",
        ));
    }
    if byte(8)? != 8 {
        return Err(refuse("a PNG that is not 8 bits per component"));
    }

    if byte(10)? != 0 {
        return Err(refuse(
            "a PNG declaring a compression method that is not deflate",
        ));
    }
    if byte(11)? != 0 {
        return Err(refuse(
            "a PNG declaring a filter method PDF's predictor does not implement",
        ));
    }
    let colour = match byte(9)? {
        0 => ColourSpace::Gray,
        2 => ColourSpace::Rgb,
        3 => {
            return Err(refuse(
                "a palette PNG: it needs an indexed colour space this version does not build",
            ));
        }
        4 | 6 => {
            return Err(refuse(
                "a PNG with an alpha channel: separating alpha into a soft mask means \
                 decoding it",
            ));
        }
        _ => return Err(refuse("a PNG with an unknown colour type")),
    };
    Ok((width, height, colour))
}

fn inflates_to_exactly(zlib: &[u8], expected: u64, row: u64) -> bool {
    use miniz_oxide::inflate::TINFLStatus;
    use miniz_oxide::inflate::core::{
        DecompressorOxide, decompress,
        inflate_flags::{TINFL_FLAG_HAS_MORE_INPUT, TINFL_FLAG_PARSE_ZLIB_HEADER},
    };

    let mut state = DecompressorOxide::new();

    let mut window = vec![0_u8; 32 * 1024];
    let mut read = 0_usize;
    let mut written = 0_u64;

    loop {
        let Some(rest) = zlib.get(read..) else {
            return false;
        };

        let flags = TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_HAS_MORE_INPUT;

        let before = written;
        let (status, consumed, produced) = decompress(&mut state, rest, &mut window, 0, flags);
        read = read.saturating_add(consumed);
        written = match u64::try_from(produced).map(|out| written.saturating_add(out)) {
            Ok(total) => total,
            Err(_) => return false,
        };

        if written > expected {
            return false;
        }

        if !row_heads_are_filters(&window, before, produced, row) {
            return false;
        }
        match status {
            TINFLStatus::Done => return written == expected,
            TINFLStatus::HasMoreOutput | TINFLStatus::NeedsMoreInput => {
                if consumed == 0 && produced == 0 {
                    return false;
                }
            }
            TINFLStatus::FailedCannotMakeProgress
            | TINFLStatus::BadParam
            | TINFLStatus::Adler32Mismatch
            | TINFLStatus::Failed => return false,
        }
    }
}

fn row_heads_are_filters(window: &[u8], before: u64, produced: usize, row: u64) -> bool {
    if row == 0 {
        return false;
    }
    let Ok(produced) = u64::try_from(produced) else {
        return false;
    };

    let offset = before % row;
    let mut at = if offset == 0 {
        before
    } else {
        before.saturating_add(row - offset)
    };
    while at < before.saturating_add(produced) {
        let Ok(index) = usize::try_from(at - before) else {
            return false;
        };
        match window.get(index) {
            Some(0..=4) => {}
            _ => return false,
        }
        at = at.saturating_add(row);
    }
    true
}

fn crc32(tag: [u8; 4], data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in tag.iter().chain(data.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
