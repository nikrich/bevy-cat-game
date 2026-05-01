//! Dev cheats. `INFINITE_RESOURCES` short-circuits inventory consumption
//! across both Build and Decoration modes so we can iterate on placement
//! physics without juggling crafting recipes. Flip to `false` (or wire to
//! a `Cheats` resource) when shipping.

pub const INFINITE_RESOURCES: bool = true;
