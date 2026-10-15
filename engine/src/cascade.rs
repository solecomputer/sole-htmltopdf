use crate::contracts::{
    ComputedStyle, Dom, Error, FontSize, SpacingEdges, StyledNode, StyledTree, Warning,
};
use crate::css::{self, Declared, FlexContainerUpdate, FlexItemUpdate, Property, Sampled};
use crate::dom::{NON_RENDERED, Node, NodeKind};

mod core;
mod diagnostics;
mod initial;
mod precedence;
mod specified;
mod walk;

pub(crate) use core::{cascade, cascade_with_author_sheets};

use diagnostics::{Diagnostics, UNIMPLEMENTED_ATTRIBUTES};
use initial::{INITIAL_FONT_SIZE, INITIAL_MARGIN};
use precedence::ordered_declarations;
use specified::{SpecifiedEdges, SpecifiedPaint, apply};
use walk::Walk;
