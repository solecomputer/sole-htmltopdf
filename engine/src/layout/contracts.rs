use crate::contracts::{BorderEdges, Error, FlowUnits, Length};

pub(crate) struct Sink<'a> {
    pub(crate) units: &'a mut FlowUnits,

    pub(crate) usable: crate::contracts::UsableHeight,

    pub(crate) sheet: crate::contracts::UsableWidth,

    pub(crate) escaped: &'a mut Escapes,

    pub(crate) overrun: &'a mut (usize, f32),

    pub(crate) refitted: &'a mut (usize, f32),

    pub(crate) decorations: &'a mut Vec<Decoration>,

    pub(crate) ignored_borders: &'a mut usize,

    pub(crate) not_enclosing: &'a mut usize,

    pub(crate) trimmed_leading: &'a mut (usize, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnBorder {
    Emit,

    CallerRecords,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Decoration {
    top: Length,
    height: Length,
    left: Length,
    width: Length,
    border: BorderEdges,

    members: Vec<usize>,
}

const COORDINATE_TOLERANCE: f32 = 0.01;

impl Decoration {
    pub(crate) fn new(
        top: Option<Length>,
        bottom: Length,
        left: Length,
        width: Length,
        border: BorderEdges,
        members: Vec<usize>,
        units: &FlowUnits,
    ) -> Result<Self, Error> {
        if members.is_empty() {
            return Err(Error::DecorationWithoutMembers);
        }

        let mut span: Option<(f32, f32)> = None;
        for member in &members {
            let Some(unit) = units.as_slice().get(*member) else {
                return Err(Error::DecorationExtentDisagreesWithMembers {
                    top: top.unwrap_or(bottom).get(),
                    bottom: bottom.get(),
                    span_top: f32::NAN,
                    span_bottom: f32::NAN,
                });
            };
            let (unit_top, unit_bottom) =
                (unit.top().get(), unit.top().get() + unit.height().get());
            span = Some(match span {
                None => (unit_top, unit_bottom),
                Some((lowest, highest)) => (lowest.min(unit_top), highest.max(unit_bottom)),
            });
        }
        let (span_top, span_bottom) = span.unwrap_or((bottom.get(), bottom.get()));

        let top = match top {
            Some(given) => given,
            None => Length::try_new(span_top, "a decoration's top")?,
        };
        let height = Length::try_new(bottom.get() - top.get(), "a decoration's height")?;

        if top.get() > span_top + COORDINATE_TOLERANCE
            || bottom.get() < span_bottom - COORDINATE_TOLERANCE
        {
            return Err(Error::DecorationExtentDisagreesWithMembers {
                top: top.get(),
                bottom: bottom.get(),
                span_top,
                span_bottom,
            });
        }
        Ok(Self {
            top,
            height,
            left,
            width,
            border,
            members,
        })
    }

    pub(crate) const fn top(&self) -> Length {
        self.top
    }

    pub(crate) const fn height(&self) -> Length {
        self.height
    }

    pub(crate) const fn left(&self) -> Length {
        self.left
    }

    pub(crate) const fn width(&self) -> Length {
        self.width
    }

    pub(crate) const fn border(&self) -> BorderEdges {
        self.border
    }

    pub(crate) fn members(&self) -> &[usize] {
        &self.members
    }

    pub(crate) fn rebased(
        self,
        offset: f32,
        members: &[usize],
        units: &FlowUnits,
    ) -> Result<Self, Error> {
        let top = Length::try_new(
            self.top.get() + offset,
            "rebasing a decoration into its row",
        )?;
        let bottom = top.get() + self.height.get();

        let (band_top, band_bottom) = members.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(lo, hi), index| match units.as_slice().get(*index) {
                Some(unit) => (
                    lo.min(unit.top().get()),
                    hi.max(unit.top().get() + unit.height().get()),
                ),
                None => (f32::NAN, f32::NAN),
            },
        );
        if !(top.get() >= band_top - COORDINATE_TOLERANCE
            && bottom <= band_bottom + COORDINATE_TOLERANCE)
        {
            return Err(Error::DecorationExtentDisagreesWithMembers {
                top: top.get(),
                bottom,
                span_top: band_top,
                span_bottom: band_bottom,
            });
        }
        Ok(Self {
            top,
            members: members.to_vec(),
            ..self
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum EscapeCause {
    DeclaredWidth,

    Displacement,

    #[default]
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Overshoot {
    pub(crate) distance: f32,
    pub(crate) edge: crate::contracts::SheetEdge,
    pub(crate) cause: EscapeCause,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Escapes {
    count: usize,
    worst: Option<Overshoot>,
}

impl Escapes {
    pub(crate) fn record(
        &mut self,
        painted_left: f32,
        painted_right: f32,
        sheet: crate::contracts::UsableWidth,
        declared: bool,
    ) {
        let past_left = sheet.left() - painted_left;
        let past_right = painted_right - sheet.right();
        if past_left <= 0.0 && past_right <= 0.0 {
            return;
        }
        self.count = self.count.saturating_add(1);

        let cause = if past_left > 0.0 {
            EscapeCause::Displacement
        } else if declared {
            EscapeCause::DeclaredWidth
        } else {
            EscapeCause::Solved
        };
        let (distance, edge) = if past_left >= past_right {
            (past_left, crate::contracts::SheetEdge::Left)
        } else {
            (past_right, crate::contracts::SheetEdge::Right)
        };

        if self.worst.is_none_or(|w| distance > w.distance) {
            self.worst = Some(Overshoot {
                distance,
                edge,
                cause,
            });
        }
    }

    pub(crate) const fn count(self) -> usize {
        self.count
    }

    pub(crate) const fn worst(self) -> Option<Overshoot> {
        self.worst
    }
}
