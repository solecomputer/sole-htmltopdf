use super::Error;
use std::fmt;

fn mm_to_points(mm: f32) -> f32 {
    mm * 72.0 / 25.4
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Length(f32);

impl Length {
    pub const ZERO: Self = Self(0.0);

    pub fn try_new(value: f32, context: &'static str) -> Result<Self, Error> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(Error::NonFiniteMeasurement { context })
        }
    }

    #[must_use]
    pub const fn from_finite(value: f32) -> Self {
        assert!(value.is_finite(), "from_finite requires a finite literal");
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl fmt::Display for Length {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}pt", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct FontSize(f32);

impl FontSize {
    pub const FURNITURE_DEFAULT: Self = Self(12.0);

    pub fn try_new(value: f32) -> Result<Self, Error> {
        if value.is_finite() && value > 0.0 && value <= crate::css::MAX_LENGTH_POINTS {
            Ok(Self(value))
        } else {
            Err(Error::InvalidFontSize { value })
        }
    }

    pub const DEFAULT: Self = Self(12.0);

    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    width: Length,
    height: Length,
}

impl PageSize {
    pub fn by_name(name: &str) -> Result<Self, Error> {
        let (width_mm, height_mm) = match name.to_ascii_uppercase().as_str() {
            "A3" => (297.0, 420.0),
            "A4" => (210.0, 297.0),
            "A5" => (148.0, 210.0),
            "LETTER" => (215.9, 279.4),
            "LEGAL" => (215.9, 355.6),
            "TABLOID" => (279.4, 431.8),
            _ => {
                return Err(Error::UnknownPageSize {
                    name: name.to_owned(),
                });
            }
        };
        Self::try_new(mm_to_points(width_mm), mm_to_points(height_mm))
    }

    #[must_use]
    pub const fn landscape(self) -> Self {
        Self {
            width: self.height,
            height: self.width,
        }
    }

    #[must_use]
    pub fn a4() -> Self {
        Self {
            width: Length(595.276),
            height: Length(841.89),
        }
    }

    pub fn try_new(width: f32, height: f32) -> Result<Self, Error> {
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            Ok(Self {
                width: Length(width),
                height: Length(height),
            })
        } else {
            Err(Error::InvalidPageSize { width, height })
        }
    }

    #[must_use]
    pub const fn width(self) -> Length {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> Length {
        self.height
    }
}
