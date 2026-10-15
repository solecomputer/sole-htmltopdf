use super::contracts::{Origin, Rule, Sheet};
use super::declarations::parse_declarations;
use super::scan::{matching_brace, without_comments};
use crate::contracts::Evaluated;

pub(crate) fn parse_sheet(source: &str, origin: Origin, medium: Evaluated) -> Sheet {
    let mut sheet = Sheet::default();
    let source = without_comments(source);

    let mut stack: Vec<Frame> = vec![Frame {
        rest: source.as_str(),
        admitted: true,
    }];

    while let Some(frame) = stack.last_mut() {
        let Some(open) = frame.rest.find('{') else {
            stack.pop();
            continue;
        };
        let (raw_prelude, tail) = frame.rest.split_at(open);
        let prelude = raw_prelude.trim();

        if let Some(semicolon) = raw_prelude.find(';') {
            let (statement, _) = raw_prelude.split_at(semicolon);
            if statement.trim_start().starts_with('@') {
                sheet.refused_selectors.record(statement.trim());
                frame.rest = frame
                    .rest
                    .get(semicolon.saturating_add(1)..)
                    .unwrap_or_default();
                continue;
            }
        }

        let Some(close) = matching_brace(tail) else {
            sheet.unterminated = true;
            stack.pop();
            continue;
        };
        let block = tail.get(1..close).unwrap_or_default();
        let admitted = frame.admitted;
        frame.rest = tail.get(close + 1..).unwrap_or_default();

        if prelude.starts_with('@') {
            let name = prelude.trim_start_matches('@').trim();
            if name.eq_ignore_ascii_case("font-face") {
                if !admitted {
                    continue;
                }
                append_font_face(&mut sheet, block);
                continue;
            }

            if name.eq_ignore_ascii_case("page") {
                if !admitted {
                    continue;
                }
                append_page(&mut sheet, block);
                continue;
            }
            if is_page_selector(name) {
                if admitted {
                    sheet.page_selectors.record(prelude);
                }
                continue;
            }

            let end = name
                .find(|c: char| c.is_whitespace() || c == '(')
                .unwrap_or(name.len());
            let (head, list) = name.split_at(end);
            if let Some(list) = head.eq_ignore_ascii_case("media").then_some(list) {
                let inner = admitted
                    && super::media::admits(
                        list,
                        medium,
                        &mut sheet.media_unsupported,
                        &mut sheet.media_malformed,
                    ) == super::media::Admits::Yes;
                if stack.len() >= MAX_GROUPING_DEPTH {
                    sheet.media_too_deep = sheet.media_too_deep.saturating_add(1);
                    continue;
                }
                stack.push(Frame {
                    rest: block,
                    admitted: inner,
                });
                continue;
            }

            sheet.refused_selectors.record(prelude);
            continue;
        }

        if !admitted {
            continue;
        }

        match crate::selector::parse_list(prelude) {
            Ok(selectors) => {
                let parsed = parse_declarations(block);
                sheet.unsupported.merge(&parsed.unsupported);
                sheet.malformed.merge(&parsed.malformed);
                sheet.rules.push(Rule {
                    origin,
                    selectors,
                    declarations: parsed.declarations,
                });
            }
            Err(()) => sheet.refused_selectors.record(prelude),
        }
    }
    sheet
}

fn append_font_face(sheet: &mut Sheet, block: &str) {
    match super::font_face::parse(block) {
        super::font_face::ParsedFace::Rule(face, unsupported) => {
            sheet.font_faces.push(face);
            for what in unsupported {
                sheet.unsupported.record(&what);
            }
        }
        super::font_face::ParsedFace::Malformed => sheet.malformed.record("@font-face"),
        super::font_face::ParsedFace::Unsupported(what) => {
            sheet.unsupported.record(&what);
        }
    }
}

fn append_page(sheet: &mut Sheet, block: &str) {
    let rules = super::page::parse_page_block(block);

    super::page::merge_furniture(&mut sheet.page.furniture, rules.furniture);
    sheet.page.author.merge(rules.author);
    sheet.unsupported.merge(&rules.unsupported);
    sheet.malformed.merge(&rules.malformed);
    sheet.page.seen = true;
    sheet.page.refused.merge(&rules.refused);
    sheet.page.translated.merge(&rules.translated);
}

fn is_page_selector(name: &str) -> bool {
    name.get(..4)
        .is_some_and(|head| head.eq_ignore_ascii_case("page"))
        && name
            .get(4..)
            .is_some_and(|tail| tail.starts_with(char::is_whitespace) || tail.starts_with(':'))
}

struct Frame<'a> {
    rest: &'a str,
    admitted: bool,
}

const MAX_GROUPING_DEPTH: usize = 32;
