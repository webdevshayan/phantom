use serde::{Deserialize, Serialize};

use crate::domain::tab::TabId;

/// Identifies one pane (a viewport over one tab) inside a window.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub struct PaneId(pub u64);

/// Direction of a split. `Horizontal` places panes side by side (left/right),
/// `Vertical` stacks them (top/bottom).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// Split tree of a window's content area. Leaves are panes holding one tab
/// each; inner nodes are splits with a first/second child and a ratio.
///
/// Invariants kept by construction:
/// - a tree is never empty (an empty window is the session's problem)
/// - every pane appears exactly once
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutTree {
    Pane {
        pane: PaneId,
        tab: TabId,
    },
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutTree>,
        second: Box<LayoutTree>,
    },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("pane {0:?} is not part of this layout")]
    UnknownPane(PaneId),
}

/// Split ratios are clamped so no pane can collapse to nothing.
pub const MIN_RATIO: f32 = 0.2;
pub const MAX_RATIO: f32 = 0.8;

/// Axis-aligned rectangle in logical pixels, used to place webview surfaces.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RectPx {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutTree {
    pub fn pane(pane: PaneId, tab: TabId) -> Self {
        LayoutTree::Pane { pane, tab }
    }

    /// First pane — the focus root of a restored window.
    pub fn first_pane(&self) -> PaneId {
        match self {
            LayoutTree::Pane { pane, .. } => *pane,
            LayoutTree::Split { first, .. } => first.first_pane(),
        }
    }

    pub fn panes(&self) -> Vec<PaneId> {
        match self {
            LayoutTree::Pane { pane, .. } => vec![*pane],
            LayoutTree::Split { first, second, .. } => {
                let mut out = first.panes();
                out.extend(second.panes());
                out
            }
        }
    }

    pub fn pane_count(&self) -> usize {
        self.panes().len()
    }

    /// (pane, tab) pairs in tree order; drives webview reconciliation.
    pub fn pane_tab_pairs(&self) -> Vec<(PaneId, TabId)> {
        match self {
            LayoutTree::Pane { pane, tab } => vec![(*pane, *tab)],
            LayoutTree::Split { first, second, .. } => {
                let mut out = first.pane_tab_pairs();
                out.extend(second.pane_tab_pairs());
                out
            }
        }
    }

    /// Every tab currently visible in a pane.
    pub fn tab_ids(&self) -> Vec<TabId> {
        self.pane_tab_pairs()
            .into_iter()
            .map(|(_, tab)| tab)
            .collect()
    }

    pub fn contains_pane(&self, target: PaneId) -> bool {
        self.panes().contains(&target)
    }

    pub fn tab_for_pane(&self, target: PaneId) -> Option<TabId> {
        match self {
            LayoutTree::Pane { pane, tab } if pane == &target => Some(*tab),
            LayoutTree::Pane { .. } => None,
            LayoutTree::Split { first, second, .. } => first
                .tab_for_pane(target)
                .or_else(|| second.tab_for_pane(target)),
        }
    }

    /// Splits `target` and inserts `new_pane` (showing `new_tab`) next to it.
    /// Consumes and returns the new tree so the shape is always valid.
    pub fn split(
        self,
        target: PaneId,
        direction: SplitDirection,
        new_pane: PaneId,
        new_tab: TabId,
    ) -> Result<LayoutTree, LayoutError> {
        match self {
            LayoutTree::Pane { pane, tab } if pane == target => Ok(LayoutTree::Split {
                direction,
                ratio: 0.5,
                first: Box::new(LayoutTree::Pane { pane, tab }),
                second: Box::new(LayoutTree::Pane {
                    pane: new_pane,
                    tab: new_tab,
                }),
            }),
            LayoutTree::Pane { .. } => Err(LayoutError::UnknownPane(target)),
            LayoutTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                if first.contains_pane(target) {
                    Ok(LayoutTree::Split {
                        direction,
                        ratio,
                        first: Box::new(first.split(target, direction, new_pane, new_tab)?),
                        second,
                    })
                } else if second.contains_pane(target) {
                    Ok(LayoutTree::Split {
                        direction,
                        ratio,
                        first,
                        second: Box::new(second.split(target, direction, new_pane, new_tab)?),
                    })
                } else {
                    Err(LayoutError::UnknownPane(target))
                }
            }
        }
    }

    /// Removes `target`, lifting its sibling. Returns `None` when the target
    /// was the only pane — the session turns that into "close the window".
    pub fn close(self, target: PaneId) -> Result<Option<LayoutTree>, LayoutError> {
        match self {
            LayoutTree::Pane { pane, .. } if pane == target => Ok(None),
            LayoutTree::Pane { .. } => Err(LayoutError::UnknownPane(target)),
            LayoutTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                if first.contains_pane(target) {
                    match first.close(target)? {
                        Some(new_first) => Ok(Some(LayoutTree::Split {
                            direction,
                            ratio,
                            first: Box::new(new_first),
                            second,
                        })),
                        None => Ok(Some(*second)),
                    }
                } else if second.contains_pane(target) {
                    match second.close(target)? {
                        Some(new_second) => Ok(Some(LayoutTree::Split {
                            direction,
                            ratio,
                            first,
                            second: Box::new(new_second),
                        })),
                        None => Ok(Some(*first)),
                    }
                } else {
                    Err(LayoutError::UnknownPane(target))
                }
            }
        }
    }

    /// Replaces the tab shown in `target`.
    pub fn set_tab(&mut self, target: PaneId, tab: TabId) -> Result<(), LayoutError> {
        match self {
            LayoutTree::Pane { pane, tab: slot } if *pane == target => {
                *slot = tab;
                Ok(())
            }
            LayoutTree::Pane { .. } => Err(LayoutError::UnknownPane(target)),
            LayoutTree::Split { first, second, .. } => {
                if first.contains_pane(target) {
                    first.set_tab(target, tab)
                } else {
                    second.set_tab(target, tab)
                }
            }
        }
    }

    /// Next pane in tree order after `current`, wrapping around. Used for
    /// keyboard cycling between panes.
    pub fn next_pane(&self, current: PaneId) -> PaneId {
        let panes = self.panes();
        let index = panes
            .iter()
            .position(|pane| pane == &current)
            .map_or(0, |i| (i + 1) % panes.len());
        panes[index]
    }

    /// Pane rectangles for a content area of `width` x `height` logical
    /// pixels. Splits allocate the first child along the split axis.
    pub fn compute_rects(&self, width: f32, height: f32) -> Vec<(PaneId, RectPx)> {
        let mut out = Vec::with_capacity(self.pane_count());
        self.collect_rects(0.0, 0.0, width, height, &mut out);
        out
    }

    fn collect_rects(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        out: &mut Vec<(PaneId, RectPx)>,
    ) {
        match self {
            LayoutTree::Pane { pane, .. } => out.push((
                *pane,
                RectPx {
                    x,
                    y,
                    width,
                    height,
                },
            )),
            LayoutTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO);
                match direction {
                    SplitDirection::Horizontal => {
                        let first_width = width * ratio;
                        first.collect_rects(x, y, first_width, height, out);
                        second.collect_rects(x + first_width, y, width - first_width, height, out);
                    }
                    SplitDirection::Vertical => {
                        let first_height = height * ratio;
                        first.collect_rects(x, y, width, first_height, out);
                        second.collect_rects(
                            x,
                            y + first_height,
                            width,
                            height - first_height,
                            out,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> LayoutTree {
        LayoutTree::pane(PaneId(1), TabId(10))
    }

    #[test]
    fn split_inserts_pane_and_keeps_counts() {
        let tree = tree()
            .split(PaneId(1), SplitDirection::Horizontal, PaneId(2), TabId(20))
            .unwrap();
        assert_eq!(tree.pane_count(), 2);
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);
        assert_eq!(tree.tab_for_pane(PaneId(1)), Some(TabId(10)));
        assert_eq!(tree.tab_for_pane(PaneId(2)), Some(TabId(20)));
    }

    #[test]
    fn nested_split_targets_deep_pane() {
        let tree = tree()
            .split(PaneId(1), SplitDirection::Horizontal, PaneId(2), TabId(20))
            .unwrap()
            .split(PaneId(2), SplitDirection::Vertical, PaneId(3), TabId(30))
            .unwrap();
        assert_eq!(tree.pane_count(), 3);
        assert_eq!(tree.tab_ids(), vec![TabId(10), TabId(20), TabId(30)]);
    }

    #[test]
    fn split_unknown_pane_errors() {
        assert_eq!(
            tree().split(PaneId(99), SplitDirection::Horizontal, PaneId(2), TabId(20)),
            Err(LayoutError::UnknownPane(PaneId(99)))
        );
    }

    #[test]
    fn close_collapses_sibling_and_last_pane_reports_none() {
        let tree = tree()
            .split(PaneId(1), SplitDirection::Horizontal, PaneId(2), TabId(20))
            .unwrap();
        // closing the first pane lifts the second
        let tree = tree.close(PaneId(1)).unwrap().unwrap();
        assert_eq!(tree.panes(), vec![PaneId(2)]);
        // closing the last remaining pane empties the window
        assert_eq!(tree.close(PaneId(2)).unwrap(), None);
    }

    #[test]
    fn set_tab_and_next_pane() {
        let mut tree = tree()
            .split(PaneId(1), SplitDirection::Vertical, PaneId(2), TabId(20))
            .unwrap();
        tree.set_tab(PaneId(2), TabId(21)).unwrap();
        assert_eq!(tree.tab_for_pane(PaneId(2)), Some(TabId(21)));
        assert!(tree.set_tab(PaneId(9), TabId(1)).is_err());
        assert_eq!(tree.next_pane(PaneId(2)), PaneId(1));
        assert_eq!(tree.next_pane(PaneId(1)), PaneId(2));
    }

    #[test]
    fn geometry_tiles_without_gaps() {
        let tree = tree()
            .split(PaneId(1), SplitDirection::Horizontal, PaneId(2), TabId(20))
            .unwrap()
            .split(PaneId(2), SplitDirection::Vertical, PaneId(3), TabId(30))
            .unwrap();
        let rects = tree.compute_rects(400.0, 300.0);
        let total_width: f32 = rects.iter().map(|(_, r)| r.width).sum();
        assert!((total_width - 400.0).abs() < 0.001);
        for (_, r) in &rects {
            assert!((r.height - 300.0).abs() < 0.001);
            assert!(
                r.width >= 400.0 * MIN_RATIO * 0.5 - 0.001,
                "pane too narrow: {r:?}"
            );
        }
        // no two panes overlap
        for (i, (_, a)) in rects.iter().enumerate() {
            for (_, b) in rects.iter().skip(i + 1) {
                let overlap_x = (a.x - b.x).abs() < a.width.min(b.width);
                let overlap_y = (a.y - b.y).abs() < a.height.min(b.height);
                assert!(!(overlap_x && overlap_y), "panes overlap: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn serde_roundtrip_keeps_shape() {
        let tree = tree()
            .split(PaneId(1), SplitDirection::Horizontal, PaneId(2), TabId(20))
            .unwrap();
        let json = serde_json::to_string(&tree).unwrap();
        assert!(json.contains("\"kind\":\"pane\""), "{json}");
        assert!(json.contains("\"kind\":\"split\""), "{json}");
        let restored: LayoutTree = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, tree);
    }

    #[test]
    fn first_pane_is_stable_focus_root() {
        let tree = tree()
            .split(PaneId(1), SplitDirection::Horizontal, PaneId(2), TabId(20))
            .unwrap();
        assert_eq!(tree.first_pane(), PaneId(1));
    }
}
