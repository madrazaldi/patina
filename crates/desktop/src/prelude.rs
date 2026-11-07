//! What nearly every view needs: GPUI, the styling and component traits, and common components.
#![allow(unused_imports)]

pub use gpui_kit::assets::IconName;
pub use gpui_kit::component::{Collapsible as _, Selectable as _, StyledExt as _};
pub use gpui_kit::component::button::{Button, ButtonVariants as _};
pub use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
pub use gpui_kit::prelude::FluentBuilder as _;
pub use gpui_kit::*;
