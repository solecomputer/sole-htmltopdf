use super::blocks::Collapse;
use crate::contracts::{
    Band, BoxNode, Error, FlowFragment, FlowUnit, FontSize, InlineContent, Length, TextAlign,
};
use crate::encoding;
use crate::metrics::Face;

mod pieces;

pub(super) struct BoxStyle {
    pub(super) link: Option<crate::contracts::LinkTarget>,
    pub(super) face: Face,
    pub(super) authored_face: Option<crate::font::AuthoredFace>,
    pub(super) font_size: FontSize,
    pub(super) line_height: Length,
    pub(super) text_align: TextAlign,
    pub(super) colour: crate::contracts::Rgb,
    pub(super) surface: crate::contracts::Surface,

    pub(super) band: crate::contracts::Band,
}

pub(super) struct Run {
    pub(super) link: Option<crate::contracts::LinkTarget>,
    pub(super) text: String,
    pub(super) face: Face,
    pub(super) authored_face: Option<crate::font::AuthoredFace>,
    pub(super) font_size: FontSize,
    pub(super) line_height: Length,
    pub(super) colour: crate::contracts::Rgb,
}

pub(super) enum InlinePiece {
    Run(Run),

    Flex(Box<BoxNode>),

    Break,

    Image(
        crate::contracts::PlacedImage,
        Option<crate::contracts::LinkTarget>,
    ),
}

impl Run {
    fn ink_extent(&self) -> (f32, f32) {
        self.authored_face.as_ref().map_or_else(
            || self.face.ink_extent(self.font_size.get()),
            |face| face.ink_extent(self.font_size.get()),
        )
    }
}

impl Run {
    pub(super) fn of(style: &BoxStyle, text: String) -> Self {
        Self {
            link: style.link.clone(),
            text,
            face: style.face,
            authored_face: style.authored_face.clone(),
            font_size: style.font_size,
            line_height: style.line_height,
            colour: style.colour,
        }
    }

    pub(super) fn width_of(&self, text: &str) -> f32 {
        self.position(text).0
    }

    fn position(&self, text: &str) -> (f32, Option<crate::font::PositionedRun>) {
        self.authored_face.as_ref().map_or_else(
            || {
                (
                    self.face
                        .measure(&encoding::to_winansi(text).bytes, self.font_size.get()),
                    None,
                )
            },
            |face| {
                let positioned = face.shape(text, self.font_size.get());
                (
                    positioned.as_ref().map_or_else(
                        || {
                            self.face
                                .measure(&encoding::to_winansi(text).bytes, self.font_size.get())
                        },
                        |run| run.advance,
                    ),
                    positioned,
                )
            },
        )
    }
}

pub(super) fn gather(node: BoxNode, out: &mut Vec<InlinePiece>) {
    if node.display == crate::contracts::Display::InlineFlex {
        out.push(InlinePiece::Flex(Box::new(node)));
        return;
    }
    let style = BoxStyle {
        link: node.link.clone(),
        face: node.face,
        authored_face: node.authored_face.clone(),
        font_size: node.font_size,
        line_height: node.line_height,
        text_align: node.text_align,
        colour: node.colour,

        surface: node.surface,

        band: crate::contracts::Band::Canvas,
    };
    let BoxNode {
        content, children, ..
    } = node;

    #[allow(
        clippy::match_same_arms,
        reason = "distinct cases whose bodies coincide; merging couples them"
    )]
    match content {
        InlineContent::Text(text) => out.push(InlinePiece::Run(Run::of(&style, text))),

        InlineContent::ForcedBreak => out.push(InlinePiece::Break),

        InlineContent::Replaced(_) => {}

        InlineContent::Placed(image) => out.push(InlinePiece::Image(image, style.link)),
        InlineContent::Empty => {}
    }
    for child in children {
        gather(child, out);
    }
}

pub(super) struct Token {
    pub(super) run: usize,
    pub(super) word: String,

    pub(super) space_before: bool,

    pub(super) space_owner: Option<usize>,

    space_run: Option<usize>,
    pub(super) word_advance: f32,
    word_positioned: Option<crate::font::PositionedRun>,
    pub(super) space_advance: f32,
    space_positioned: Option<crate::font::PositionedRun>,
}

pub(super) fn tokenize(runs: &[Run]) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut space_pending: Option<usize> = None;
    for (index, run) in runs.iter().enumerate() {
        debug_assert!(
            run.link
                .as_ref()
                .is_none_or(|target| !target.as_str().is_empty()),
            "only validated non-empty targets reach layout"
        );

        if run.text.starts_with(|c: char| c.is_ascii_whitespace()) {
            space_pending.get_or_insert(index);
        }
        for (position, word) in run.text.split_ascii_whitespace().enumerate() {
            let owner = if position > 0 {
                Some(index)
            } else {
                space_pending
            };
            let (word_advance, word_positioned) = run.position(word);
            let space_run = tokens.last().map(|token| token.run).or(owner);
            let (space_advance, space_positioned) = space_run
                .and_then(|space_run| runs.get(space_run))
                .map_or((0.0, None), |owner| owner.position(" "));
            tokens.push(Token {
                run: index,
                word: word.to_owned(),
                space_before: owner.is_some() && !tokens.is_empty(),
                space_owner: owner,
                space_run,
                word_advance,
                word_positioned,
                space_advance,
                space_positioned,
            });
            space_pending = None;
        }
        if run.text.ends_with(|c: char| c.is_ascii_whitespace()) {
            space_pending.get_or_insert(index);
        }
    }
    tokens
}

pub(super) fn emit_blank_line(
    block: &BoxStyle,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    pending.flush_into(cursor);
    let height = block.line_height.get();
    let unit = FlowUnit::line(
        Length::try_new(*cursor, "a blank line's top")?,
        Length::try_new(height, "a blank line's height")?,
        block.band,
        Vec::new(),
    )?;
    sink.units.push(unit)?;
    *cursor += height;
    Ok(())
}

fn emit_image(
    image: (
        crate::contracts::PlacedImage,
        Option<crate::contracts::LinkTarget>,
    ),
    block: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    let (image, link) = image;

    pending.flush_into(cursor);

    let (used_width, used_height, refit) = fit_into(image.width, image.height, available);

    let total = image.acquired_scale * refit;
    if total < 1.0 {
        sink.refitted.0 = sink.refitted.0.saturating_add(1);
        if total < sink.refitted.1 {
            sink.refitted.1 = total;
        }
    }
    let top = Length::try_new(*cursor, "an image's top")?;
    let height = Length::try_new(used_height, "an image's used height")?;
    let flow_image = crate::contracts::FlowImage {
        asset: image.asset,
        x: Length::try_new(left, "an image's left edge")?,
        y: top,
        width: Length::try_new(used_width, "an image's used width")?,
        height,
    };
    let flow_link = link.and_then(|target| {
        crate::contracts::FlowLink::new(
            target,
            flow_image.x,
            flow_image.y,
            flow_image.width,
            flow_image.height,
        )
    });
    let unit = FlowUnit::image_with_link(top, height, block.band, flow_image, flow_link)?;
    sink.units.push(unit)?;
    *cursor += used_height;
    Ok(())
}

fn fit_into(width: f32, height: f32, available: f32) -> (f32, f32, f32) {
    if available <= 0.0 || width <= available {
        return (width, height, 1.0);
    }
    let scale = available / width;

    (available, height * scale, scale)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_pieces(
    pieces: &mut Vec<InlinePiece>,
    block: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    if pieces
        .iter()
        .any(|piece| matches!(piece, InlinePiece::Flex(_)))
    {
        return emit_mixed_pieces(pieces, block, left, available, cursor, pending, sink);
    }
    let gathered = std::mem::take(pieces);
    let mut runs: Vec<Run> = Vec::new();
    for piece in gathered {
        match piece {
            InlinePiece::Run(run) => runs.push(run),

            InlinePiece::Flex(_) => {}

            InlinePiece::Image(image, link) => {
                emit_lines(&mut runs, block, left, available, cursor, pending, sink)?;
                emit_image((image, link), block, left, available, cursor, pending, sink)?;
            }
            InlinePiece::Break => {
                if tokenize(&runs).is_empty() {
                    runs.clear();
                    emit_blank_line(block, cursor, pending, sink)?;
                } else {
                    emit_lines(&mut runs, block, left, available, cursor, pending, sink)?;
                }
            }
        }
    }
    emit_lines(&mut runs, block, left, available, cursor, pending, sink)
}

enum MixedItem {
    Word {
        run: usize,
        word: String,
        space_before: bool,
    },
    Atom {
        node: Box<BoxNode>,
        width: f32,
        space_before: bool,
        space_width: f32,
    },
}

impl MixedItem {
    fn width(&self, runs: &[Run], first: bool) -> f32 {
        match self {
            Self::Word {
                run,
                word,
                space_before,
            } => runs.get(*run).map_or(0.0, |style| {
                let mut text = String::new();
                if !first && *space_before {
                    text.push(' ');
                }
                text.push_str(word);
                style.width_of(&text)
            }),
            Self::Atom {
                width,
                space_before,
                space_width,
                ..
            } => {
                width
                    + if !first && *space_before {
                        *space_width
                    } else {
                        0.0
                    }
            }
        }
    }
}

struct LaidAtom {
    units: Vec<FlowUnit>,
    decorations: Vec<super::contracts::Decoration>,
    height: f32,

    baseline: f32,
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the mixed inline producer receives the same flow context as the ordinary inline producer it replaces and owns one exhaustive source-order dispatch"
)]
fn emit_mixed_pieces(
    pieces: &mut Vec<InlinePiece>,
    block: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    let mut runs = Vec::new();
    let mut line = Vec::new();
    let mut line_width = 0.0_f32;
    let mut whitespace_pending = false;
    for piece in std::mem::take(pieces) {
        match piece {
            InlinePiece::Run(run) => {
                let run_index = runs.len();
                let text = run.text.clone();
                runs.push(run);
                whitespace_pending |= text.starts_with(|c: char| c.is_ascii_whitespace());
                for (position, word) in text.split_ascii_whitespace().enumerate() {
                    let item = MixedItem::Word {
                        run: run_index,
                        word: word.to_owned(),
                        space_before: whitespace_pending || position > 0,
                    };
                    push_mixed(
                        item,
                        &mut line,
                        &mut line_width,
                        &runs,
                        block,
                        left,
                        available,
                        cursor,
                        pending,
                        sink,
                    )?;
                    whitespace_pending = false;
                }
                whitespace_pending |= text.ends_with(|c: char| c.is_ascii_whitespace());
            }
            InlinePiece::Flex(node) => {
                let space_width = runs.last().map_or(0.0, |run| run.width_of(" "));
                let item = MixedItem::Atom {
                    width: super::flex::inline_width(&node, available)?,
                    node,
                    space_before: whitespace_pending,
                    space_width,
                };
                push_mixed(
                    item,
                    &mut line,
                    &mut line_width,
                    &runs,
                    block,
                    left,
                    available,
                    cursor,
                    pending,
                    sink,
                )?;
                whitespace_pending = false;
            }
            InlinePiece::Break => {
                if line.is_empty() {
                    emit_blank_line(block, cursor, pending, sink)?;
                } else {
                    place_mixed_line(
                        std::mem::take(&mut line),
                        line_width,
                        &runs,
                        block,
                        left,
                        available,
                        cursor,
                        pending,
                        sink,
                    )?;
                    line_width = 0.0;
                }
                whitespace_pending = false;
            }
            InlinePiece::Image(image, image_link) => {
                if !line.is_empty() {
                    place_mixed_line(
                        std::mem::take(&mut line),
                        line_width,
                        &runs,
                        block,
                        left,
                        available,
                        cursor,
                        pending,
                        sink,
                    )?;
                    line_width = 0.0;
                }
                emit_image(
                    (image, image_link),
                    block,
                    left,
                    available,
                    cursor,
                    pending,
                    sink,
                )?;
                whitespace_pending = false;
            }
        }
    }
    if !line.is_empty() {
        place_mixed_line(
            line, line_width, &runs, block, left, available, cursor, pending, sink,
        )?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "line breaking needs the candidate, current line, immutable styles and the ordinary flow context"
)]
fn push_mixed(
    item: MixedItem,
    line: &mut Vec<MixedItem>,
    line_width: &mut f32,
    runs: &[Run],
    block: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    let addition = item.width(runs, line.is_empty());
    if !line.is_empty() && *line_width + addition > available {
        place_mixed_line(
            std::mem::take(line),
            *line_width,
            runs,
            block,
            left,
            available,
            cursor,
            pending,
            sink,
        )?;
        *line_width = 0.0;
    }
    *line_width += item.width(runs, line.is_empty());
    line.push(item);
    Ok(())
}

enum ReadyMixed {
    Word { run: usize, text: String, x: f32 },
    Atom(LaidAtom),
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "a line owns its source items, styles, containing geometry, cursor and output lanes; preparation and consumption stay paired at this loss-sensitive seam"
)]
fn place_mixed_line(
    line: Vec<MixedItem>,
    line_width: f32,
    runs: &[Run],
    block: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    pending.flush_into(cursor);
    let alignment = match block.text_align {
        TextAlign::Left => 0.0,
        TextAlign::Right => available - line_width,
        TextAlign::Center => (available - line_width) / 2.0,
    };
    record_if_line_overruns(sink, left + alignment, line_width);

    let mut x = left + alignment;
    let mut ready = Vec::with_capacity(line.len());
    let mut text_height = 0.0_f32;
    for (position, item) in line.into_iter().enumerate() {
        match item {
            MixedItem::Word {
                run,
                word,
                space_before,
            } => {
                let Some(style) = runs.get(run) else {
                    continue;
                };
                let mut text = String::new();
                if position > 0 && space_before {
                    text.push(' ');
                }
                text.push_str(&word);
                let width = style.width_of(&text);
                text_height = text_height.max(style.line_height.get());
                ready.push(ReadyMixed::Word { run, text, x });
                x += width;
            }
            MixedItem::Atom {
                node,
                width,
                space_before,
                space_width,
            } => {
                if position > 0 && space_before {
                    x += space_width;
                }
                ready.push(ReadyMixed::Atom(lay_atom(*node, x, width, sink)?));
                x += width;
            }
        }
    }
    let text_baseline = ready
        .iter()
        .filter_map(|item| match item {
            ReadyMixed::Word { run, .. } => runs.get(*run),
            ReadyMixed::Atom(_) => None,
        })
        .map(|run| {
            run.authored_face.as_ref().map_or_else(
                || run.face.baseline_offset(run.font_size.get(), text_height),
                |face| face.baseline_offset(run.font_size.get(), text_height),
            )
        })
        .fold(0.0_f32, f32::max);
    let line_baseline = ready
        .iter()
        .fold(text_baseline, |baseline, item| match item {
            ReadyMixed::Word { .. } => baseline,
            ReadyMixed::Atom(atom) => baseline.max(atom.baseline),
        });
    let below_baseline = ready.iter().fold(
        (text_height - text_baseline).max(0.0),
        |below, item| match item {
            ReadyMixed::Word { .. } => below,
            ReadyMixed::Atom(atom) => below.max((atom.height - atom.baseline).max(0.0)),
        },
    );
    let line_height = line_baseline + below_baseline;
    let text_offset = line_baseline - text_baseline;
    let mut flows = Vec::with_capacity(ready.len());
    let mut decorations = Vec::new();
    for item in ready {
        match item {
            ReadyMixed::Word { run, text, x } => {
                let Some(style) = runs.get(run) else {
                    continue;
                };
                let width = style.width_of(&text);
                let fragment = FlowFragment {
                    text: text.clone(),
                    x: Length::try_new(x, "placing text beside an inline flex atom")?,
                    y: Length::try_new(text_baseline, "placing an atomic inline baseline")?,
                    line_height: Length::try_new(text_height, "placing an atomic inline line")?,
                    font_size: style.font_size,
                    face: style.face,
                    positioned: style
                        .authored_face
                        .as_ref()
                        .and_then(|face| face.shape(&text, style.font_size.get())),
                    colour: style.colour,
                };
                let mut links = Vec::new();
                push_link_region(&mut links, style, x, text_baseline, width)?;
                let unit = FlowUnit::line_with_links(
                    Length::try_new(0.0, "an atomic inline text top")?,
                    Length::try_new(text_height, "an atomic inline text height")?,
                    Band::Canvas,
                    vec![fragment],
                    links,
                )?;
                flows.push(crate::contracts::FlexItemFlow::new(vec![unit], text_offset));
            }
            ReadyMixed::Atom(mut atom) => {
                let offset = line_baseline - atom.baseline;
                flows.push(crate::contracts::FlexItemFlow::new(atom.units, offset));
                decorations.extend(atom.decorations.drain(..).map(|item| (item, offset)));
            }
        }
    }
    let top = *cursor;
    sink.units.push(FlowUnit::inline_line(
        Length::try_new(top, "the top of a line with an inline flex atom")?,
        Length::try_new(line_height, "the height of a line with an inline flex atom")?,
        block.band,
        flows,
    )?)?;
    let member = [sink.units.len().saturating_sub(1)];
    for (decoration, offset) in decorations {
        match decoration.rebased(top + offset, &member, sink.units) {
            Ok(rebased) => sink.decorations.push(rebased),
            Err(Error::DecorationExtentDisagreesWithMembers { .. }) => {
                *sink.not_enclosing = sink.not_enclosing.saturating_add(1);
            }
            Err(other) => return Err(other),
        }
    }
    *cursor += line_height;
    Ok(())
}

fn lay_atom(
    node: BoxNode,
    left: f32,
    width: f32,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<LaidAtom, Error> {
    let mut units = crate::contracts::FlowUnits::new();
    let mut decorations = Vec::new();
    let mut cursor = 0.0_f32;
    let mut pending = Collapse::default();
    let mut local_sink = super::contracts::Sink {
        units: &mut units,
        usable: sink.usable,
        sheet: sink.sheet,
        escaped: sink.escaped,
        overrun: sink.overrun,
        refitted: sink.refitted,
        decorations: &mut decorations,
        ignored_borders: sink.ignored_borders,
        not_enclosing: sink.not_enclosing,
        trimmed_leading: sink.trimmed_leading,
    };
    super::blocks::place(
        node,
        crate::contracts::UsableWidth::span(left, width)?,
        &mut cursor,
        &mut pending,
        &mut local_sink,
        super::contracts::OwnBorder::Emit,
    )?;
    pending.flush_into(&mut cursor);
    let baseline = units
        .fragments()
        .next()
        .map_or(cursor, |fragment| fragment.y.get());
    Ok(LaidAtom {
        units: units.take_units(),
        decorations,
        height: cursor,
        baseline,
    })
}

pub(super) fn emit_lines(
    runs: &mut Vec<Run>,
    block: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    let gathered = std::mem::take(runs);
    let tokens = tokenize(&gathered);
    if tokens.is_empty() {
        return Ok(());
    }

    pending.flush_into(cursor);

    let width_of = |token: &Token| -> f32 {
        let space = if token.space_before {
            token.space_advance
        } else {
            0.0
        };
        space + token.word_advance
    };

    let mut line: Vec<&Token> = Vec::new();
    let mut line_width = 0.0_f32;
    for token in &tokens {
        let addition = width_of(token);
        if !line.is_empty() && line_width + addition > available {
            place_line(
                &line, line_width, &gathered, block, left, available, cursor, sink,
            )?;
            line.clear();
            line_width = 0.0;
        }

        line_width += if line.is_empty() {
            token.word_advance
        } else {
            addition
        };
        line.push(token);
    }
    if !line.is_empty() {
        place_line(
            &line, line_width, &gathered, block, left, available, cursor, sink,
        )?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "a line needs its tokens, its runs, its block's alignment, and its position; bundling them into a struct would name the same values twice"
)]
fn place_line(
    line: &[&Token],
    line_width: f32,
    runs: &[Run],
    block: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    sink: &mut super::contracts::Sink<'_>,
) -> Result<(), Error> {
    let offset = match block.text_align {
        TextAlign::Left => 0.0,
        TextAlign::Right => available - line_width,
        TextAlign::Center => (available - line_width) / 2.0,
    };

    record_if_line_overruns(sink, left + offset, line_width);

    let line_height = line
        .iter()
        .filter_map(|token| runs.get(token.run))
        .map(|run| run.line_height.get())
        .fold(0.0_f32, f32::max);
    let baseline_offset = line
        .iter()
        .filter_map(|token| runs.get(token.run))
        .map(|run| {
            run.authored_face.as_ref().map_or_else(
                || run.face.baseline_offset(run.font_size.get(), line_height),
                |face| face.baseline_offset(run.font_size.get(), line_height),
            )
        })
        .fold(0.0_f32, f32::max);
    let baseline = *cursor + baseline_offset;

    let usable = sink.usable.get();
    let (ink_above, ink_below) = line
        .iter()
        .filter_map(|token| runs.get(token.run))
        .map(Run::ink_extent)
        .fold((0.0_f32, 0.0_f32), |(a, b), (above, below)| {
            (a.max(above), b.max(below))
        });
    let ink_height = ink_above + ink_below;

    let leading_is_empty = matches!(block.band, Band::Canvas);
    let (line_height, baseline, trimmed) =
        if leading_is_empty && line_height > usable && ink_height <= usable {
            (ink_height, *cursor + ink_above, line_height - ink_height)
        } else {
            (line_height, baseline, 0.0)
        };
    if trimmed > 0.0 {
        sink.trimmed_leading.0 = sink.trimmed_leading.0.saturating_add(1);
        sink.trimmed_leading.1 += trimmed;
    }

    let links = build_line_links(line, runs, left + offset, baseline)?;

    let pieces = pieces::assemble(line);

    let mut fragments = Vec::new();
    let mut x = left + offset;
    for (run_index, text, positioned, width) in pieces {
        let Some(run) = runs.get(run_index) else {
            continue;
        };
        fragments.push(FlowFragment {
            text,
            x: Length::try_new(x, "placing a fragment")?,
            y: Length::try_new(baseline, "placing a fragment")?,
            line_height: Length::try_new(line_height, "placing a line")?,
            font_size: run.font_size,
            face: run.face,
            positioned,
            colour: run.colour,
        });
        x += width;
    }
    sink.units.push(FlowUnit::line_with_links(
        Length::try_new(*cursor, "the top of a line box")?,
        Length::try_new(line_height, "the height of a line box")?,
        block.band,
        fragments,
        links,
    )?)?;

    *cursor += line_height;
    Ok(())
}

fn build_line_links(
    line: &[&Token],
    runs: &[Run],
    mut x: f32,
    baseline: f32,
) -> Result<Vec<crate::contracts::FlowLink>, Error> {
    let mut links = Vec::new();
    for (position, token) in line.iter().enumerate() {
        if position > 0 && token.space_before {
            let width = token.space_advance;
            if let Some(owner) = token.space_owner.and_then(|index| runs.get(index)) {
                push_link_region(&mut links, owner, x, baseline, width)?;
            }
            x += width;
        }
        if let Some(run) = runs.get(token.run) {
            let width = token.word_advance;
            push_link_region(&mut links, run, x, baseline, width)?;
            x += width;
        }
    }
    Ok(links)
}

fn push_link_region(
    links: &mut Vec<crate::contracts::FlowLink>,
    owner: &Run,
    x: f32,
    baseline: f32,
    width: f32,
) -> Result<(), Error> {
    let Some(target) = owner.link.clone() else {
        return Ok(());
    };
    let (above, below) = owner.ink_extent();
    let Some(next) = crate::contracts::FlowLink::new(
        target,
        Length::try_new(x, "placing a link's left edge")?,
        Length::try_new(baseline - above, "placing a link's top edge")?,
        Length::try_new(width, "placing a link's width")?,
        Length::try_new(above + below, "placing a link's height")?,
    ) else {
        return Ok(());
    };
    if let Some(previous) = links.last_mut()
        && previous.merge_adjacent(&next)?
    {
        return Ok(());
    }
    links.push(next);
    Ok(())
}

fn record_if_line_overruns(sink: &mut super::contracts::Sink<'_>, start: f32, advance: f32) {
    let past_right = start + advance - sink.sheet.right();
    let past_left = sink.sheet.left() - start;
    let worst = past_right.max(past_left);
    if worst <= 0.0 {
        return;
    }
    sink.overrun.0 = sink.overrun.0.saturating_add(1);
    if worst > sink.overrun.1 {
        sink.overrun.1 = worst;
    }
}
