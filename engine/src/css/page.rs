use crate::contracts::{
    AuthorPage, AuthorSize, BoxContent, Edges, Furniture, Length, MarginBand, PageEdge, PageSize,
    PageToken, Piece, Rgb, Slot, VerticalPlacement,
};

use super::contracts::Sampled;
use super::declarations::{
    EdgeError, drive_declarations, finish_declaration, parse_edges, parse_length_value,
};

#[derive(Debug, Default)]
pub(crate) struct PageRules {
    pub(crate) furniture: Furniture,
    pub(crate) author: AuthorPage,
    pub(crate) unsupported: Sampled,
    pub(crate) malformed: Sampled,

    pub(crate) refused: Sampled,

    pub(crate) translated: Sampled,

    pub(crate) seen: bool,
}

const PLACED: [(&str, Edge, usize); 6] = [
    ("top-left", Edge::Top, 0),
    ("top-center", Edge::Top, 1),
    ("top-right", Edge::Top, 2),
    ("bottom-left", Edge::Bottom, 0),
    ("bottom-center", Edge::Bottom, 1),
    ("bottom-right", Edge::Bottom, 2),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Bottom,
}

pub(crate) fn merge_furniture(into: &mut Furniture, from: Furniture) {
    merge_band(&mut into.top, from.top);
    merge_band(&mut into.bottom, from.bottom);
}

fn merge_band(into: &mut Option<MarginBand>, from: Option<MarginBand>) {
    let Some(from) = from else {
        return;
    };
    let Some(existing) = into.as_mut() else {
        *into = Some(from);
        return;
    };

    for (into, incoming) in existing.content.iter_mut().zip(from.content.clone()) {
        if !incoming.is_empty() {
            *into = incoming;
        }
    }

    existing.spacing = from.spacing;
    existing.face = from.face;
    existing.font_size = from.font_size;
    existing.colour = from.colour;
    existing.placement = from.placement;
    if from.rule.is_some() {
        existing.rule = from.rule;
    }
}

pub(crate) fn parse_page_block(block: &str) -> PageRules {
    let mut out = PageRules {
        seen: true,
        ..PageRules::default()
    };
    let mut rest = block;
    let mut declarations = String::new();

    while let Some(at) = super::scan::find_outside_strings(rest, '@') {
        declarations.push_str(rest.get(..at).unwrap_or_default());
        let tail = rest.get(at..).unwrap_or_default();
        let Some(open) = super::scan::find_outside_strings(tail, '{') else {
            break;
        };
        let name = tail.get(1..open).unwrap_or_default().trim().to_owned();

        let Some(close) = super::scan::matching_brace(tail.get(open..).unwrap_or_default()) else {
            break;
        };
        let body = tail.get(open + 1..open + close).unwrap_or_default();
        apply_box(&name, body, &mut out);
        rest = tail.get(open + close + 1..).unwrap_or_default();
    }
    declarations.push_str(rest);
    let mut collector = PageCollector { out: &mut out };
    let malformed = drive_declarations(&declarations, &mut collector);
    for _ in 0..malformed {
        out.malformed.record("@page declaration");
    }
    out
}

struct PageCollector<'a> {
    out: &'a mut PageRules,
}

impl<'i> cssparser::DeclarationParser<'i> for PageCollector<'_> {
    type Declaration = ();
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        parser: &mut cssparser::Parser<'i, 't>,
        _start: &cssparser::ParserState,
    ) -> Result<(), cssparser::ParseError<'i, ()>> {
        let lowered = name.to_ascii_lowercase();
        match lowered.as_str() {
            "size" => self.parse_size(parser),
            "margin" => self.parse_margin(parser),
            "margin-top" => self.parse_margin_side(PageEdge::Top, parser),
            "margin-right" => self.parse_margin_side(PageEdge::Right, parser),
            "margin-bottom" => self.parse_margin_side(PageEdge::Bottom, parser),
            "margin-left" => self.parse_margin_side(PageEdge::Left, parser),
            _ => {
                self.out.unsupported.record(&format!("@page {lowered}"));
                drain(parser);
                Ok(())
            }
        }
    }
}

impl PageCollector<'_> {
    fn parse_size<'i>(
        &mut self,
        parser: &mut cssparser::Parser<'i, '_>,
    ) -> Result<(), cssparser::ParseError<'i, ()>> {
        if let Ok(name) = parser.try_parse(cssparser::Parser::expect_ident_cloned) {
            let lowered = name.to_ascii_lowercase();
            let value = match lowered.as_str() {
                "auto" => Some(AuthorSize::Auto),
                "a4" => PageSize::by_name("A4").ok().map(AuthorSize::Page),
                "letter" => PageSize::by_name("Letter").ok().map(AuthorSize::Page),
                _ => None,
            };
            let Some(value) = value else {
                self.out.unsupported.record(&format!("@page size: {name}"));
                drain(parser);
                return Ok(());
            };
            if !parser.is_exhausted()
                && parser
                    .try_parse(|input| {
                        let orientation = input.expect_ident_cloned()?;
                        if orientation.eq_ignore_ascii_case("portrait")
                            || orientation.eq_ignore_ascii_case("landscape")
                        {
                            Ok(())
                        } else {
                            Err(input.new_custom_error::<(), ()>(()))
                        }
                    })
                    .is_ok()
            {
                let _ = finish_declaration(parser)?;
                self.out
                    .unsupported
                    .record("@page size: orientation keyword");
                return Ok(());
            }
            let important = finish_declaration(parser)?;
            self.out.author.set_size(value, important);
            return Ok(());
        }

        let first = parser
            .try_parse(parse_length_value)
            .map_err(|_| parser.new_custom_error::<(), ()>(()))?;
        let Ok(second) = parser.try_parse(parse_length_value) else {
            let _ = finish_declaration(parser)?;
            self.out.unsupported.record("@page size: one length");
            return Ok(());
        };
        if parser
            .try_parse(|input| {
                let orientation = input.expect_ident_cloned()?;
                if orientation.eq_ignore_ascii_case("portrait")
                    || orientation.eq_ignore_ascii_case("landscape")
                {
                    Ok(())
                } else {
                    Err(input.new_custom_error::<(), ()>(()))
                }
            })
            .is_ok()
        {
            let _ = finish_declaration(parser)?;
            self.out
                .unsupported
                .record("@page size: orientation keyword");
            return Ok(());
        }
        let important = finish_declaration(parser)?;
        if first.em != 0.0
            || second.em != 0.0
            || first.points <= 0.0
            || second.points <= 0.0
            || first.points > second.points
        {
            self.out
                .unsupported
                .record("@page size: unsupported dimensions");
            return Ok(());
        }
        let size = PageSize::try_new(first.points, second.points)
            .map_err(|_| parser.new_custom_error::<(), ()>(()))?;
        self.out.author.set_size(AuthorSize::Page(size), important);
        Ok(())
    }

    fn parse_margin<'i>(
        &mut self,
        parser: &mut cssparser::Parser<'i, '_>,
    ) -> Result<(), cssparser::ParseError<'i, ()>> {
        let spacing = match parse_edges(parser) {
            Ok(value) => value,
            Err(EdgeError::Auto) => {
                self.out.unsupported.record("@page margin: auto");
                drain(parser);
                return Ok(());
            }
            Err(EdgeError::Malformed) => {
                return Err(parser.new_custom_error::<(), ()>(()));
            }
        };
        let important = finish_declaration(parser)?;
        let Some(edges) = absolute_edges(spacing) else {
            self.out
                .unsupported
                .record("@page margin: unsupported length");
            return Ok(());
        };
        self.out
            .author
            .set_margin(PageEdge::Top, edges.top, important);
        self.out
            .author
            .set_margin(PageEdge::Right, edges.right, important);
        self.out
            .author
            .set_margin(PageEdge::Bottom, edges.bottom, important);
        self.out
            .author
            .set_margin(PageEdge::Left, edges.left, important);
        Ok(())
    }

    fn parse_margin_side<'i>(
        &mut self,
        edge: PageEdge,
        parser: &mut cssparser::Parser<'i, '_>,
    ) -> Result<(), cssparser::ParseError<'i, ()>> {
        if parser
            .try_parse(|input| input.expect_ident_matching("auto"))
            .is_ok()
        {
            self.out.unsupported.record("@page margin: auto");
            drain(parser);
            return Ok(());
        }
        let spacing = parse_length_value(parser)?;
        let important = finish_declaration(parser)?;
        if spacing.em != 0.0 || spacing.points < 0.0 {
            self.out
                .unsupported
                .record("@page margin: unsupported length");
            return Ok(());
        }
        self.out.author.set_margin(
            edge,
            Length::try_new(spacing.points, "an authored page margin")
                .map_err(|_| parser.new_custom_error::<(), ()>(()))?,
            important,
        );
        Ok(())
    }
}

fn absolute_edges(spacing: crate::contracts::SpacingEdges) -> Option<Edges> {
    let length = |value: crate::contracts::Spacing| {
        (value.em == 0.0 && value.points >= 0.0)
            .then(|| Length::try_new(value.points, "an authored page margin").ok())
            .flatten()
    };
    Some(Edges {
        top: length(spacing.top)?,
        right: length(spacing.right)?,
        bottom: length(spacing.bottom)?,
        left: length(spacing.left)?,
    })
}

fn drain(parser: &mut cssparser::Parser<'_, '_>) {
    while parser.next().is_ok() {}
}

impl cssparser::AtRuleParser<'_> for PageCollector<'_> {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
}

impl cssparser::QualifiedRuleParser<'_> for PageCollector<'_> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();
}

impl cssparser::RuleBodyItemParser<'_, (), ()> for PageCollector<'_> {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

fn apply_box(name: &str, body: &str, out: &mut PageRules) {
    let lowered = name.to_ascii_lowercase();
    let name = lowered.as_str();
    let Some((_, edge, slot)) = PLACED
        .iter()
        .copied()
        .find(|(box_name, _, _)| *box_name == name)
    else {
        out.refused.record(name);
        return;
    };
    let declared = declarations_of(body, name, &mut out.translated);
    let Some(content) = declared.content else {
        return;
    };
    let band = match edge {
        Edge::Top => &mut out.furniture.top,
        Edge::Bottom => &mut out.furniture.bottom,
    };
    let target = band.get_or_insert_with(|| MarginBand {
        content: [
            BoxContent::default(),
            BoxContent::default(),
            BoxContent::default(),
        ],
        rule: None,

        placement: VerticalPlacement::Centred,
        face: declared.face.unwrap_or(crate::metrics::Face::Helvetica),
        font_size: declared
            .size
            .and_then(|size| crate::contracts::FontSize::try_new(size).ok())
            .unwrap_or(crate::contracts::FontSize::DEFAULT),

        colour: declared.colour.unwrap_or(Rgb::BLACK),
        spacing: Length::from_finite(0.0),
    });

    let conflict = |what: &str, out: &mut Sampled| {
        out.record(&format!(
            "@{name} {{ {what} }} — one band carries one {what} for all three \
             slots in this version, and a later box on the same band wins"
        ));
    };
    if let Some(face) = declared.face
        && face != target.face
        && !target.slots().all(Slot::is_empty)
    {
        conflict("font-family", &mut out.translated);
    }
    if let Some(size) = declared.size
        && (size - target.font_size.get()).abs() > f32::EPSILON
        && !target.slots().all(Slot::is_empty)
    {
        conflict("font-size", &mut out.translated);
    }
    if let Some(colour) = declared.colour
        && colour != target.colour
        && !target.slots().all(Slot::is_empty)
    {
        conflict("color", &mut out.translated);
    }
    if let Some(face) = declared.face {
        target.face = face;
    }
    if let Some(size) = declared
        .size
        .and_then(|s| crate::contracts::FontSize::try_new(s).ok())
    {
        target.font_size = size;
    }
    if let Some(colour) = declared.colour {
        target.colour = colour;
    }
    if let Some(existing) = target.slot_mut(slot) {
        *existing = content;
    }
}

#[derive(Default)]
struct Declared {
    content: Option<Slot>,
    face: Option<crate::metrics::Face>,
    size: Option<f32>,
    colour: Option<Rgb>,
}

fn declarations_of(body: &str, box_name: &str, translated: &mut Sampled) -> Declared {
    let mut out = Declared::default();

    let mut note = |declaration: &str, value: &str| {
        translated.record(&format!("@{box_name} {{ {declaration}: {value} }}"));
    };
    for declaration in body.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        match name {
            "content" => out.content = Some(parse_content(value)),
            "font-family" => {
                out.face = crate::metrics::Face::by_name(value.trim_matches(['"', '\''].as_ref()));
                if out.face.is_none() {
                    note(name, value);
                }
            }
            "font-size" => {
                out.size = parse_points(value);
                if out.size.is_none() {
                    note(name, value);
                }
            }
            "color" => {
                out.colour = parse_colour(value);
                if out.colour.is_none() {
                    note(name, value);
                }
            }

            _ => note(name, value),
        }
    }
    out
}

fn parse_content(value: &str) -> Slot {
    let mut pieces = Vec::new();
    let mut rest = value.trim();
    while !rest.is_empty() {
        rest = rest.trim_start();

        if let Some((quote, after)) = rest
            .strip_prefix('"')
            .map(|after| ('"', after))
            .or_else(|| rest.strip_prefix('\'').map(|after| ('\'', after)))
        {
            let end = after.find(quote).unwrap_or(after.len());
            let text = after.get(..end).unwrap_or_default();
            if !text.is_empty() {
                pieces.push(Piece::Literal(text.to_owned()));
            }
            rest = after.get(end + 1..).unwrap_or_default();
        } else if let Some(after) = rest.strip_prefix("counter(") {
            let end = after.find(')').unwrap_or(after.len());
            let inside = after.get(..end).unwrap_or_default().trim();

            let (name, style) = match inside.split_once(',') {
                Some((name, style)) => (name.trim(), Some(style.trim())),
                None => (inside, None),
            };
            let style_is_drawable = style.is_none_or(|style| style == "decimal");
            match name {
                _ if !style_is_drawable => {
                    pieces.push(Piece::Token(PageToken::Unanswerable("counter style")));
                }
                "page" => pieces.push(Piece::Token(PageToken::Page)),
                "pages" => pieces.push(Piece::Token(PageToken::Pages)),

                _ => pieces.push(Piece::Token(PageToken::Unanswerable("counter"))),
            }
            rest = after.get(end + 1..).unwrap_or_default();
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            rest = rest.get(end..).unwrap_or_default();
        }
    }
    Slot::new(pieces)
}

fn parse_points(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    let number = trimmed.strip_suffix("pt").unwrap_or(trimmed);
    let parsed: f32 = number.trim().parse().ok()?;
    crate::contracts::FontSize::try_new(parsed)
        .ok()
        .map(crate::contracts::FontSize::get)
}

fn parse_colour(value: &str) -> Option<Rgb> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#')?;

    let channel = |at: usize| {
        u8::from_str_radix(hex.get(at..at + 2)?, 16)
            .ok()
            .map(|byte| f32::from(byte) / 255.0)
    };
    match hex.len() {
        6 => Rgb::try_new(channel(0)?, channel(2)?, channel(4)?).ok(),
        _ => None,
    }
}
