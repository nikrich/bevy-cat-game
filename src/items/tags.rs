use serde::{Deserialize, Serialize};

/// Bit flags describing what an item is for. Used by systems that want to
/// query "all placeables" or "all foods" without hardcoding lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemTags(pub u32);

impl ItemTags {
    pub const NONE: Self = Self(0);
    pub const PLACEABLE: Self = Self(1 << 0);
    pub const STACKABLE: Self = Self(1 << 1);
    pub const RAW: Self = Self(1 << 2);
    pub const REFINED: Self = Self(1 << 3);
    pub const FURNITURE: Self = Self(1 << 4);
    pub const STRUCTURAL: Self = Self(1 << 5);
    pub const FOOD: Self = Self(1 << 6);
    pub const DECORATION: Self = Self(1 << 7);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for ItemTags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for ItemTags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
