//! Wrapping (job 6.1, CON-A4-F2-P4): which layers of a wall turn the corner at its free ends and
//! at the sides of the doors and windows in it, set by the wall type ([`Wrapping`], stored in its
//! system block from FORMAT.md version 2) with a wall's own choice over it, and where each turned
//! layer lies ([`turned`]). Note CON-A4-F2-P4-wall-wrapping says why.
//!
//! The rule. A layer wraps when its own flag says so ([`crate::Layer::wraps`]; only a shell
//! layer may) and the wall wraps on its side there: at a free end (a wall end where no joint is
//! made) on the side [`AtEnds`] names, at an insert's sides on the side or sides [`AtInserts`]
//! names. A wrapping layer turns the corner and runs across the end of every layer between it and
//! the core and across the core, to the core's far face; where both sides wrap at an insert, each
//! side's turned layers run to the core's middle, and the two meet there. Wrapping layers on one
//! side nest: the outermost turns first, and each one inside it turns inside the one before. Every
//! layer a turned layer covers is set back from the end or the opening's edge by the thicknesses
//! of the turned layers outside it. Layers on a side that does not wrap stop flush.
//!
//! [`turned`] gives this as depths across the wall (from its exterior finish face) and setbacks
//! along it (from the end or the opening's edge), both in grid steps, so the plan rule
//! ([`crate::joins`]) and the solid builder (construct-regen's kernel builder, at inserts) lay
//! out the same thing. Exact integers, every step checked; the core's middle, when the core is an
//! odd number of steps thick, is the step nearer the exterior. Nothing here is interim.

use serde::{Deserialize, Serialize};

/// Which side's layers wrap at a wall's free ends. The stored names are the variant names in
/// snake case.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
pub enum AtEnds {
    /// No layer wraps: every layer stops flush with the end.
    #[default]
    None,
    /// The exterior shell's wrapping layers turn round the end.
    Exterior,
    /// The interior shell's wrapping layers turn round the end.
    Interior,
}

/// Which side's layers wrap at the sides of an insert (a door or a window). The stored names are
/// the variant names in snake case.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
pub enum AtInserts {
    /// No layer wraps: every layer is cut flush with the opening.
    #[default]
    None,
    /// The exterior shell's wrapping layers turn into the opening.
    Exterior,
    /// The interior shell's wrapping layers turn into the opening.
    Interior,
    /// Both shells' wrapping layers turn into the opening, meeting at the core's middle.
    Both,
}

impl AtEnds {
    /// Every choice, in its order.
    pub const ALL: [Self; 3] = [Self::None, Self::Exterior, Self::Interior];

    /// Its stored name.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Exterior => "exterior",
            Self::Interior => "interior",
        }
    }

    /// The choice a stored name names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.key() == key)
    }

    /// Which sides wrap: exterior, interior.
    #[must_use]
    pub const fn sides(self) -> (bool, bool) {
        match self {
            Self::None => (false, false),
            Self::Exterior => (true, false),
            Self::Interior => (false, true),
        }
    }
}

impl AtInserts {
    /// Every choice, in its order.
    pub const ALL: [Self; 4] = [Self::None, Self::Exterior, Self::Interior, Self::Both];

    /// Its stored name.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Exterior => "exterior",
            Self::Interior => "interior",
            Self::Both => "both",
        }
    }

    /// The choice a stored name names.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.key() == key)
    }

    /// Which sides wrap: exterior, interior.
    #[must_use]
    pub const fn sides(self) -> (bool, bool) {
        match self {
            Self::None => (false, false),
            Self::Exterior => (true, false),
            Self::Interior => (false, true),
            Self::Both => (true, true),
        }
    }
}

/// A wall type's wrapping: at its free ends and at its inserts. The default wraps nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Wrapping {
    /// At free ends.
    pub ends: AtEnds,
    /// At the sides of inserts.
    pub inserts: AtInserts,
}

impl Wrapping {
    /// Whether it wraps nothing anywhere.
    #[must_use]
    pub fn is_none(self) -> bool {
        self == Self::default()
    }

    /// This wrapping with a wall's own choices over it: each given choice replaces the type's.
    #[must_use]
    pub fn overridden(self, ends: Option<AtEnds>, inserts: Option<AtInserts>) -> Self {
        Self {
            ends: ends.unwrap_or(self.ends),
            inserts: inserts.unwrap_or(self.inserts),
        }
    }
}

/// One layer as wrapping reads it: its thickness in grid steps (0 or more) and whether it wraps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrapLayer {
    /// Its thickness, in steps.
    pub thickness: i64,
    /// Whether it wraps.
    pub wraps: bool,
}

/// A part of a layer's depth and how far it is set back there: `from` and `to` are depths from
/// the exterior finish face (`from < to`), `setback` the distance from the end or the opening's
/// edge, all in steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setback {
    /// Where the part begins, from the exterior finish face.
    pub from: i64,
    /// Where it ends.
    pub to: i64,
    /// How far the layer stops short of the end or the opening's edge there.
    pub setback: i64,
}

/// A turned part of a wrapping layer: across the depths `from..to`, between the setbacks `near`
/// (its outer face, toward the end or the opening) and `far`, all in steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Turn {
    /// The wrapping layer it is part of, by index from the exterior face.
    pub layer: usize,
    /// Where it begins across the wall, from the exterior finish face: the layer's own inner face.
    pub from: i64,
    /// Where it ends: the core's far face, or its middle where both sides wrap.
    pub to: i64,
    /// Its setback nearer the end or the opening.
    pub near: i64,
    /// Its setback farther in: `near` plus the layer's thickness.
    pub far: i64,
}

/// Where every layer lies at a wrapped end or insert side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Turned {
    /// Each layer's setbacks, by index from the exterior face: one part for most layers, two for
    /// a core layer that the core's middle crosses where both sides wrap; none for a layer with no
    /// thickness.
    pub setbacks: Vec<Vec<Setback>>,
    /// Each turned part, outermost first on the exterior side, then the interior side's.
    pub turns: Vec<Turn>,
}

impl Turned {
    /// Whether anything turns: when nothing does, every setback is 0.
    #[must_use]
    pub fn turns_any(&self) -> bool {
        !self.turns.is_empty()
    }
}

/// The depths of every face from the exterior finish face: `n + 1` of them.
fn depths(layers: &[WrapLayer]) -> Option<Vec<i64>> {
    let mut out = Vec::with_capacity(layers.len().checked_add(1)?);
    let mut depth = 0_i64;
    out.push(depth);
    for layer in layers {
        if layer.thickness < 0 {
            return None;
        }
        depth = depth.checked_add(layer.thickness)?;
        out.push(depth);
    }
    Some(out)
}

/// The turned parts of one side's wrapping layers, outermost first: `shell` the side's layer
/// indices from the face inward, each turned part running to depth `to`, and each layer's setback
/// from those turns written into `set`, for the depths of `span`.
fn side(
    layers: &[WrapLayer],
    faces: &[i64],
    shell: &[usize],
    to: i64,
    exterior: bool,
    turns: &mut Vec<Turn>,
) -> Option<Vec<(usize, i64)>> {
    // The setback reached so far, and each shell layer's setback.
    let mut reached = 0_i64;
    let mut set = Vec::with_capacity(shell.len());
    for &at in shell {
        let layer = layers.get(at)?;
        set.push((at, reached));
        if layer.wraps && layer.thickness > 0 {
            // Its own inner face: the face toward the core.
            let inner = if exterior {
                *faces.get(at.checked_add(1)?)?
            } else {
                *faces.get(at)?
            };
            let (from, end) = if exterior { (inner, to) } else { (to, inner) };
            let far = reached.checked_add(layer.thickness)?;
            if from < end {
                turns.push(Turn {
                    layer: at,
                    from,
                    to: end,
                    near: reached,
                    far,
                });
            }
            reached = far;
        }
    }
    set.push((usize::MAX, reached));
    Some(set)
}

/// Where every layer of a wall lies where it wraps on the sides `exterior` and `interior`: the
/// layers from the exterior face, the core the layers `core.0..core.1`. A side wraps only where
/// one of its shell layers wraps; a core layer's flag is never read. `None` for layers or a core
/// that do not fit (a negative thickness, a core out of range or empty), or a depth beyond an
/// `i64`.
#[must_use]
pub fn turned(
    layers: &[WrapLayer],
    core: (usize, usize),
    exterior: bool,
    interior: bool,
) -> Option<Turned> {
    let (first, last) = core;
    if first >= last || last > layers.len() {
        return None;
    }
    let faces = depths(layers)?;
    // A side wraps only where one of its shell layers with a thickness wraps; where only one of
    // two wrapping sides has one, it runs to the core's far face as if it were the only side.
    let wraps_in = |range: std::ops::Range<usize>| {
        layers
            .get(range)
            .is_some_and(|shell| shell.iter().any(|l| l.wraps && l.thickness > 0))
    };
    let exterior = exterior && wraps_in(0..first);
    let interior = interior && wraps_in(last..layers.len());
    let (core_out, core_in) = (*faces.get(first)?, *faces.get(last)?);
    let middle = core_out.checked_add(core_in.checked_sub(core_out)?.div_euclid(2))?;
    let (ext_to, int_to) = match (exterior, interior) {
        (true, true) => (middle, middle),
        _ => (core_in, core_out),
    };
    let mut turns = Vec::new();
    let mut setbacks: Vec<Vec<Setback>> = vec![Vec::new(); layers.len()];
    let mut put = |at: usize, from: i64, to: i64, setback: i64| {
        if from < to
            && let Some(slot) = setbacks.get_mut(at)
        {
            slot.push(Setback { from, to, setback });
        }
    };
    let ext_shell: Vec<usize> = (0..first).collect();
    let int_shell: Vec<usize> = (last..layers.len()).rev().collect();
    let ext = if exterior {
        side(layers, &faces, &ext_shell, ext_to, true, &mut turns)?
    } else {
        ext_shell
            .iter()
            .map(|&at| (at, 0))
            .chain([(usize::MAX, 0)])
            .collect()
    };
    let int = if interior {
        side(layers, &faces, &int_shell, int_to, false, &mut turns)?
    } else {
        int_shell
            .iter()
            .map(|&at| (at, 0))
            .chain([(usize::MAX, 0)])
            .collect()
    };
    for &(at, setback) in ext.iter().chain(&int) {
        if at != usize::MAX {
            put(
                at,
                *faces.get(at)?,
                *faces.get(at.checked_add(1)?)?,
                setback,
            );
        }
    }
    // The core: set back by the exterior side's turns down to their depth, by the interior's
    // from theirs, and flush between where only one side wraps.
    let ext_core = ext.last().map_or(0, |&(_, s)| s);
    let int_core = int.last().map_or(0, |&(_, s)| s);
    for at in first..last {
        let (from, to) = (*faces.get(at)?, *faces.get(at.checked_add(1)?)?);
        match (exterior, interior) {
            (true, true) => {
                put(at, from, to.min(middle), ext_core);
                put(at, from.max(middle), to, int_core);
            }
            (true, false) => put(at, from, to, ext_core),
            (false, true) => put(at, from, to, int_core),
            (false, false) => put(at, from, to, 0),
        }
    }
    Some(Turned { setbacks, turns })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects, reason = "tests")]
mod tests {
    use super::*;

    fn l(thickness: i64, wraps: bool) -> WrapLayer {
        WrapLayer { thickness, wraps }
    }

    /// Cladding (wraps), a cavity, the frame (core) and a lining (wraps): wrapped on the exterior,
    /// the cladding turns across the cavity and the frame, to the frame's inner face; the cavity
    /// and the frame stop short by the cladding; the lining is flush.
    #[test]
    fn the_exterior_cladding_turns_to_the_core_s_far_face() {
        let layers = [l(20, true), l(50, false), l(140, false), l(15, true)];
        let t = turned(&layers, (2, 3), true, false).unwrap();
        assert_eq!(
            t.turns,
            vec![Turn {
                layer: 0,
                from: 20,
                to: 210,
                near: 0,
                far: 20
            }]
        );
        let s = |from, to, setback| Setback { from, to, setback };
        assert_eq!(
            t.setbacks,
            vec![
                vec![s(0, 20, 0)],
                vec![s(20, 70, 20)],
                vec![s(70, 210, 20)],
                vec![s(210, 225, 0)]
            ]
        );
    }

    /// Two wrapping layers on one side nest; on the interior the lining turns to the core's
    /// exterior face; on both sides at an insert each runs to the core's middle.
    #[test]
    fn wrapping_layers_nest_and_both_sides_meet_at_the_core_s_middle() {
        let layers = [l(20, true), l(10, true), l(100, false), l(15, true)];
        let ext = turned(&layers, (2, 3), true, false).unwrap();
        assert_eq!(ext.turns.len(), 2);
        assert_eq!((ext.turns[1].near, ext.turns[1].far), (20, 30));
        assert_eq!(
            ext.setbacks[2],
            vec![Setback {
                from: 30,
                to: 130,
                setback: 30
            }]
        );
        let int = turned(&layers, (2, 3), false, true).unwrap();
        assert_eq!(
            int.turns,
            vec![Turn {
                layer: 3,
                from: 30,
                to: 130,
                near: 0,
                far: 15
            }]
        );
        let both = turned(&layers, (2, 3), true, true).unwrap();
        assert_eq!(both.turns[0].to, 80);
        assert_eq!(both.turns[2].from, 80);
        assert_eq!(
            both.setbacks[2],
            vec![
                Setback {
                    from: 30,
                    to: 80,
                    setback: 30
                },
                Setback {
                    from: 80,
                    to: 130,
                    setback: 15
                }
            ]
        );
    }

    /// Nothing wraps where no shell layer on the side wraps, and a core out of range is refused.
    #[test]
    fn a_side_with_no_wrapping_layer_turns_nothing() {
        let layers = [l(20, false), l(100, false), l(20, false)];
        let t = turned(&layers, (1, 2), true, true).unwrap();
        assert!(!t.turns_any());
        assert!(t.setbacks.iter().flatten().all(|s| s.setback == 0));
        assert!(turned(&layers, (2, 2), true, false).is_none());
        assert!(turned(&layers, (1, 4), true, false).is_none());
    }
}
