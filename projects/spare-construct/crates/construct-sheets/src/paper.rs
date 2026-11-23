//! Paper: the standard sizes of ISO 216 (A0 to A5) in either orientation, and a custom size.
//!
//! Every paper length is construct-params' [`Length`] (whole grid steps of 1/6,400 mm), so 841 mm
//! and a custom 297.5 mm are exact and hand to the printer unrounded. Paper has its own frame:
//! the origin is the sheet's lower left corner, x runs right and y runs up.

use crate::error::SheetError;
use construct_params::Length;

/// A standard paper size of ISO 216, the A series.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaperSize {
    /// 841 by 1189 mm.
    A0,
    /// 594 by 841 mm.
    A1,
    /// 420 by 594 mm.
    A2,
    /// 297 by 420 mm.
    A3,
    /// 210 by 297 mm.
    A4,
    /// 148 by 210 mm.
    A5,
}

impl PaperSize {
    /// Every standard size, largest first.
    pub const ALL: [Self; 6] = [Self::A0, Self::A1, Self::A2, Self::A3, Self::A4, Self::A5];

    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::A0 => "a0",
            Self::A1 => "a1",
            Self::A2 => "a2",
            Self::A3 => "a3",
            Self::A4 => "a4",
            Self::A5 => "a5",
        }
    }

    /// The size a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.key() == key)
    }

    /// The short side and the long side, in whole millimetres, as ISO 216 gives them.
    #[must_use]
    pub const fn short_long_mm(self) -> (i32, i32) {
        match self {
            Self::A0 => (841, 1189),
            Self::A1 => (594, 841),
            Self::A2 => (420, 594),
            Self::A3 => (297, 420),
            Self::A4 => (210, 297),
            Self::A5 => (148, 210),
        }
    }
}

/// Which way a standard sheet is turned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    /// The long side runs across (x).
    Landscape,
    /// The long side runs up (y).
    Portrait,
}

impl Orientation {
    /// The key it is stored under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Landscape => "landscape",
            Self::Portrait => "portrait",
        }
    }

    /// The orientation a key names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "landscape" => Some(Self::Landscape),
            "portrait" => Some(Self::Portrait),
            _ => None,
        }
    }
}

/// The smallest custom side: 50 mm.
pub const MIN_CUSTOM: Length = Length::from_mm(50);
/// The largest custom side: 5 m.
pub const MAX_CUSTOM: Length = Length::from_mm(5000);

/// The paper of a sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    /// A standard size, turned one way.
    Standard {
        /// The size.
        size: PaperSize,
        /// Its orientation.
        orientation: Orientation,
    },
    /// Any width and height from [`MIN_CUSTOM`] to [`MAX_CUSTOM`], each an exact length.
    Custom {
        /// Across (x).
        width: Length,
        /// Up (y).
        height: Length,
    },
}

impl Paper {
    /// The stored key of the paper: a size's key, or `custom`.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Standard { size, .. } => size.key(),
            Self::Custom { .. } => "custom",
        }
    }

    /// The width, across.
    #[must_use]
    pub const fn width(&self) -> Length {
        match *self {
            Self::Standard { size, orientation } => {
                let (short, long) = size.short_long_mm();
                Length::from_mm(match orientation {
                    Orientation::Landscape => long,
                    Orientation::Portrait => short,
                })
            }
            Self::Custom { width, .. } => width,
        }
    }

    /// The height, up.
    #[must_use]
    pub const fn height(&self) -> Length {
        match *self {
            Self::Standard { size, orientation } => {
                let (short, long) = size.short_long_mm();
                Length::from_mm(match orientation {
                    Orientation::Landscape => short,
                    Orientation::Portrait => long,
                })
            }
            Self::Custom { height, .. } => height,
        }
    }

    /// Asks whether the paper can be a sheet's.
    ///
    /// # Errors
    /// [`SheetError::BadPaper`] for a custom side under [`MIN_CUSTOM`] or over [`MAX_CUSTOM`].
    pub fn check(&self) -> Result<(), SheetError> {
        if let Self::Custom { width, height } = self {
            for (what, side) in [("width", width), ("height", height)] {
                if *side < MIN_CUSTOM || *side > MAX_CUSTOM {
                    return Err(SheetError::BadPaper(format!(
                        "A custom paper {what} of {side} is outside {MIN_CUSTOM} to {MAX_CUSTOM}."
                    )));
                }
            }
        }
        Ok(())
    }

    /// The paper as text: `A3 landscape (420 mm by 297 mm)` or `420 mm by 297 mm`.
    #[must_use]
    pub fn describe(&self) -> String {
        let sides = format!("{} by {}", self.width(), self.height());
        match self {
            Self::Standard { size, orientation } => {
                format!(
                    "{} {} ({sides})",
                    size.key().to_uppercase(),
                    orientation.key()
                )
            }
            Self::Custom { .. } => sides,
        }
    }
}
