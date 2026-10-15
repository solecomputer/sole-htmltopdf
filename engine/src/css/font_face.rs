use cssparser::{DeclarationParser, ParserInput, RuleBodyItemParser, RuleBodyParser};

use crate::contracts::{FontStyle, FontWeight};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceRule {
    pub(crate) family: String,
    pub(crate) sources: Vec<FaceSource>,
    pub(crate) weight: FontWeight,
    pub(crate) style: FontStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FaceSource {
    Url {
        value: String,
        format: Option<String>,
    },
    Local(String),
}

pub(crate) enum ParsedFace {
    Rule(FaceRule, Vec<String>),
    Malformed,
    Unsupported(String),
}

pub(crate) fn parse(block: &str) -> ParsedFace {
    let mut collector = Collector::default();
    let mut input = ParserInput::new(block);
    let mut parser = cssparser::Parser::new(&mut input);
    let malformed = RuleBodyParser::new(&mut parser, &mut collector)
        .filter(Result::is_err)
        .count();
    if malformed > 0 {
        return ParsedFace::Malformed;
    }
    let (Some(family), Some(sources)) = (collector.family, collector.sources) else {
        return ParsedFace::Malformed;
    };
    if sources.is_empty() {
        return ParsedFace::Unsupported(
            collector
                .unsupported
                .into_iter()
                .next()
                .unwrap_or_else(|| "@font-face src".to_owned()),
        );
    }
    ParsedFace::Rule(
        FaceRule {
            family,
            sources,
            weight: collector.weight.unwrap_or(FontWeight::Normal),
            style: collector.style.unwrap_or(FontStyle::Normal),
        },
        collector.unsupported,
    )
}

#[derive(Default)]
struct Collector {
    family: Option<String>,
    sources: Option<Vec<FaceSource>>,
    weight: Option<FontWeight>,
    style: Option<FontStyle>,
    unsupported: Vec<String>,
}

impl<'i> DeclarationParser<'i> for Collector {
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
            "font-family" => {
                let family = parser
                    .try_parse(|input| input.expect_string().map(ToString::to_string))
                    .or_else(|_| parser.expect_ident().map(ToString::to_string))
                    .map_err(basic)?;
                require_end(parser)?;
                self.family = Some(family);
            }
            "src" => {
                let (sources, had_unsupported) = parse_sources(parser);
                if had_unsupported {
                    self.unsupported.push("@font-face src arm".to_owned());
                }
                self.sources = Some(sources);
            }
            "font-weight" => {
                let weight = parser
                    .try_parse(|input| input.expect_ident().map(|value| value.to_ascii_lowercase()))
                    .map_or_else(
                        |_| parser.expect_integer().map(|value| value.to_string()),
                        Ok,
                    )
                    .map_err(basic)?;
                if !parser.is_exhausted() {
                    self.unsupported
                        .push("@font-face font-weight range".to_owned());
                    while parser.next().is_ok() {}
                }
                self.weight = match weight.as_str() {
                    "normal" | "400" => Some(FontWeight::Normal),
                    "bold" | "700" => Some(FontWeight::Bold),
                    _ => {
                        self.unsupported
                            .push(format!("@font-face font-weight: {weight}"));
                        None
                    }
                };
            }
            "font-style" => {
                let style = parser
                    .expect_ident()
                    .map(|value| value.to_ascii_lowercase())
                    .map_err(basic)?;
                if !parser.is_exhausted() {
                    self.unsupported
                        .push("@font-face font-style value".to_owned());
                    while parser.next().is_ok() {}
                }
                self.style = match style.as_str() {
                    "normal" => Some(FontStyle::Normal),
                    "italic" => Some(FontStyle::Italic),
                    _ => {
                        self.unsupported
                            .push(format!("@font-face font-style: {style}"));
                        None
                    }
                };
            }
            other => {
                self.unsupported
                    .push(format!("@font-face descriptor: {other}"));
                while parser.next().is_ok() {}
            }
        }
        Ok(())
    }
}

fn parse_sources(parser: &mut cssparser::Parser<'_, '_>) -> (Vec<FaceSource>, bool) {
    let mut arms = 0_usize;
    let sources = parser.parse_comma_separated_ignoring_errors(|parser| {
        arms = arms.saturating_add(1);
        let source = if let Ok(local) = parser.try_parse(|input| {
            input.expect_function_matching("local")?;
            input.parse_nested_block(|nested| {
                let value = nested
                    .try_parse(|inner| inner.expect_string().map(ToString::to_string))
                    .or_else(|_| nested.expect_ident().map(ToString::to_string))
                    .map_err(basic)?;
                require_end(nested)?;
                Ok(value)
            })
        }) {
            FaceSource::Local(local)
        } else {
            let value = parser
                .expect_url()
                .map(|value| value.to_string())
                .map_err(basic)?;
            let format = parser
                .try_parse(|input| {
                    input.expect_function_matching("format")?;
                    input.parse_nested_block(|nested| {
                        let value: String = nested
                            .try_parse(|inner| inner.expect_string().map(ToString::to_string))
                            .or_else(|_| nested.expect_ident().map(ToString::to_string))
                            .map_err(basic)?;
                        require_end(nested)?;
                        Ok(value.to_ascii_lowercase())
                    })
                })
                .ok();
            FaceSource::Url { value, format }
        };
        require_end(parser)?;
        Ok(source)
    });
    let had_unsupported = sources.len() != arms;
    (sources, had_unsupported)
}

fn require_end<'i>(
    parser: &mut cssparser::Parser<'i, '_>,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    if parser.is_exhausted() {
        Ok(())
    } else {
        Err(parser.new_custom_error::<(), ()>(()))
    }
}

fn basic(error: cssparser::BasicParseError<'_>) -> cssparser::ParseError<'_, ()> {
    error.into()
}

impl cssparser::AtRuleParser<'_> for Collector {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
}

impl cssparser::QualifiedRuleParser<'_> for Collector {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();
}

impl RuleBodyItemParser<'_, (), ()> for Collector {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}
