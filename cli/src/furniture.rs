use sole_htmltopdf::{
    BoxContent, DocumentFacts, Furniture, Length, MarginBand, MarginRule, PageToken, Rgb, Slot,
    VerticalPlacement,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Timestamp {
    pub(crate) year: i32,
    pub(crate) month: u32,
    pub(crate) day: u32,
    pub(crate) hour: u32,
    pub(crate) minute: u32,
    pub(crate) second: u32,
}

impl Timestamp {
    pub(crate) fn date(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub(crate) fn time(self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

pub(crate) fn now() -> Timestamp {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rest = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    Timestamp {
        year,
        month,
        day,
        hour: u32::try_from(rest / 3_600).unwrap_or(0),
        minute: u32::try_from(rest % 3_600 / 60).unwrap_or(0),
        second: u32::try_from(rest % 60).unwrap_or(0),
    }
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        i32::try_from(if m <= 2 { y + 1 } else { y }).unwrap_or(1970),
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Raw {
    pub(crate) header: [Option<String>; 3],
    pub(crate) footer: [Option<String>; 3],
    pub(crate) header_spacing: Option<f32>,
    pub(crate) footer_spacing: Option<f32>,
    pub(crate) header_line: Option<bool>,
    pub(crate) footer_line: Option<bool>,
    pub(crate) header_font_name: Option<String>,
    pub(crate) footer_font_name: Option<String>,
    pub(crate) header_font_size: Option<f32>,
    pub(crate) footer_font_size: Option<f32>,
    pub(crate) replacements: Vec<(String, String)>,

    pub(crate) header_html: Option<std::path::PathBuf>,
    pub(crate) footer_html: Option<std::path::PathBuf>,
}

#[derive(Debug, Default)]
pub(crate) struct Desugared {
    pub(crate) furniture: Furniture,
    pub(crate) facts: DocumentFacts,

    pub(crate) notices: Vec<String>,
}

const DEFAULT_FONT_SIZE: f32 = 12.0;

const DEFAULT_SPACING: f32 = 0.0;

const RULE_THICKNESS: f32 = 0.5;

pub(crate) fn desugar(raw: &Raw, source_name: Option<&str>, clock: Timestamp) -> Desugared {
    let mut out = Desugared {
        facts: DocumentFacts {
            title: None,

            source: source_name.map(str::to_owned),
            date: Some(clock.date()),
            time: Some(clock.time()),
        },
        ..Desugared::default()
    };
    let bindings: Vec<(String, String)> = raw.replacements.clone();
    out.furniture.top = band(
        &raw.header,
        raw.header_spacing,
        raw.header_line,
        raw.header_font_name.as_deref(),
        raw.header_font_size,
        &bindings,
        source_name.is_some(),
        raw.header_html.is_some(),
        &mut out.notices,
    );
    out.furniture.bottom = band(
        &raw.footer,
        raw.footer_spacing,
        raw.footer_line,
        raw.footer_font_name.as_deref(),
        raw.footer_font_size,
        &bindings,
        source_name.is_some(),
        raw.footer_html.is_some(),
        &mut out.notices,
    );
    out
}

#[allow(
    clippy::too_many_arguments,
    reason = "one band's whole configuration; a struct here would be RawFurniture split in half"
)]
fn band(
    slots: &[Option<String>; 3],
    spacing: Option<f32>,
    line: Option<bool>,
    font_name: Option<&str>,
    font_size: Option<f32>,
    bindings: &[(String, String)],
    named_source: bool,

    carries_document: bool,
    notices: &mut Vec<String>,
) -> Option<MarginBand> {
    if slots.iter().all(Option::is_none) && !carries_document {
        return None;
    }
    let parsed = [
        parse(slots[0].as_deref(), bindings, named_source, notices),
        parse(slots[1].as_deref(), bindings, named_source, notices),
        parse(slots[2].as_deref(), bindings, named_source, notices),
    ];
    let size = font_size.unwrap_or(DEFAULT_FONT_SIZE);
    Some(MarginBand {
        content: parsed.map(BoxContent::Slot),
        rule: line.unwrap_or(false).then_some(MarginRule {
            thickness: Length::from_finite(RULE_THICKNESS),
            colour: Rgb::BLACK,
        }),

        placement: VerticalPlacement::InnerEdge,
        face: face_for(font_name, notices),
        font_size: size_or_default(size, notices),
        colour: Rgb::BLACK,
        spacing: Length::from_finite(spacing.unwrap_or(DEFAULT_SPACING)),
    })
}

fn size_or_default(size: f32, notices: &mut Vec<String>) -> sole_htmltopdf::FontSize {
    if let Ok(usable) = sole_htmltopdf::FontSize::try_new(size) {
        return usable;
    }
    notices.push(format!(
        "a header or footer font size of {size} is not usable; \
         {DEFAULT_FONT_SIZE}pt is used instead"
    ));

    sole_htmltopdf::FontSize::DEFAULT
}

fn face_for(name: Option<&str>, notices: &mut Vec<String>) -> sole_htmltopdf::Face {
    let Some(name) = name else {
        return sole_htmltopdf::Face::Helvetica;
    };
    if let Some(face) = sole_htmltopdf::Face::by_name(name) {
        return face;
    }
    notices.push(format!(
        "the face \"{name}\" is not one of the PDF base 14; Helvetica is used instead"
    ));
    sole_htmltopdf::Face::Helvetica
}

fn parse(
    text: Option<&str>,
    bindings: &[(String, String)],
    named_source: bool,
    notices: &mut Vec<String>,
) -> Slot {
    let Some(text) = text else {
        return Slot::default();
    };

    sole_htmltopdf::parse_template(text, |name| {
        token_for(name, bindings, named_source, notices)
    })
}

pub(crate) fn say(notice: &sole_htmltopdf::TemplateNotice) -> String {
    match notice {
        sole_htmltopdf::TemplateNotice::SourceHasNoName => {
            "[webpage] names the input file and this document was read from \
             standard input, which has no name; it is left empty"
                .to_owned()
        }
        sole_htmltopdf::TemplateNotice::NotComputed(named) => format!(
            "[{named}] is a wkhtmltopdf token this version does not compute; it is left empty"
        ),
        sole_htmltopdf::TemplateNotice::RebindRefused(named) => {
            format!("--replace cannot rebind the built-in token [{named}]; the built-in is used")
        }
    }
}

fn token_for(
    name: &str,
    bindings: &[(String, String)],
    named_source: bool,
    notices: &mut Vec<String>,
) -> Option<PageToken> {
    let (token, notice) = sole_htmltopdf::resolve_token(name, bindings, named_source)?;
    if let Some(notice) = notice {
        notices.push(say(&notice));
    }
    Some(token)
}
