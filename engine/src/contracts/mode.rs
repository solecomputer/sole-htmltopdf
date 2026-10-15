use super::{Edges, Error, PageSize};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum RenderMode {
    #[default]
    Modern,

    Wkhtmltopdf0126(Bundle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SmartShrinking {
    On,

    Off,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    smart_shrinking: SmartShrinking,
}

impl Bundle {
    #[must_use]
    pub const fn wkhtmltopdf_0_12_6() -> Self {
        Self {
            smart_shrinking: SmartShrinking::On,
        }
    }

    #[must_use]
    pub const fn with_smart_shrinking(mut self, choice: SmartShrinking) -> Self {
        self.smart_shrinking = choice;
        self
    }

    #[must_use]
    pub const fn smart_shrinking(&self) -> SmartShrinking {
        self.smart_shrinking
    }

    #[must_use]
    pub const fn unimplemented_members() -> &'static [&'static str] {
        &["legacy DPI mapping", "legacy table fragmentation"]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct Scale(f32);

impl Scale {
    pub(crate) const ONE: Self = Self(1.0);

    pub(crate) fn try_new(value: f32) -> Result<Self, Error> {
        if value.is_finite() && value > 0.0 {
            Ok(Self(value))
        } else {
            Err(Error::InvalidScale { value })
        }
    }

    pub(crate) const fn get(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RenderingIntent {
    pub(crate) layout_page: PageSize,

    pub(crate) layout_margin: Edges,

    pub(crate) physical_page: PageSize,

    pub(crate) output: Scale,

    pub(crate) usable_height: UsableHeight,

    pub(crate) usable_width: UsableWidth,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UsableHeight(f32);

impl UsableHeight {
    pub(crate) fn new(page: PageSize, margin: Edges) -> Result<Self, Error> {
        let height = page.height().get() - margin.top.get() - margin.bottom.get();
        if height <= 0.0 {
            return Err(Error::PageMarginsExceedPage {
                axis: crate::contracts::Axis::Vertical,
                extent: page.height().get(),
                margins: margin.top.get() + margin.bottom.get(),
            });
        }
        Ok(Self(height))
    }

    pub(crate) const fn get(self) -> f32 {
        self.0
    }

    pub(crate) fn fits(extent: f32, budget: f32) -> bool {
        extent <= budget + f32::EPSILON
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UsableWidth {
    left: f32,
    right: f32,
}

impl UsableWidth {
    pub(crate) fn new(page: PageSize, margin: Edges) -> Result<Self, Error> {
        let left = margin.left.get();
        let right = page.width().get() - margin.right.get();
        if right - left <= 0.0 {
            return Err(Error::PageMarginsExceedPage {
                axis: crate::contracts::Axis::Horizontal,
                extent: page.width().get(),
                margins: margin.left.get() + margin.right.get(),
            });
        }
        Ok(Self { left, right })
    }

    pub(crate) const fn extent(self) -> f32 {
        self.right - self.left
    }

    pub(crate) const fn left(self) -> f32 {
        self.left
    }

    pub(crate) const fn right(self) -> f32 {
        self.right
    }

    pub(crate) fn span(left: f32, extent: f32) -> Result<Self, Error> {
        let right = left + extent.max(0.0);
        if !left.is_finite() || !right.is_finite() {
            return Err(Error::NonFiniteMeasurement {
                context: "a child interval on the inline axis",
            });
        }
        Ok(Self { left, right })
    }
}
