pub struct BlendMode(pub(crate) usize);

impl BlendMode {
    pub const NONE: Self = Self(0);
    pub const ALPHA: Self = Self(1);
    pub const ADD: Self = Self(2);
    pub const SUBTRACT: Self = Self(3);
    pub const REPLACE: Self = Self(4);
    pub const MULTIPLY: Self = Self(5);
    pub const LIGHTEN: Self = Self(6);
    pub const DARKEN: Self = Self(7);
    pub const SCREEN: Self = Self(8);
    pub const END: Self = Self(9);
}