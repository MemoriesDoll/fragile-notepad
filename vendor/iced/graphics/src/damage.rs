//! Compute the damage between frames.
use crate::core::{Point, Rectangle, Vector};

/// A summary of damage regions by primitive kind.
#[derive(Debug, Clone, Copy, Default)]
pub struct Summary {
    /// The number of damaged quads.
    pub quads: usize,
    /// The number of damaged text primitives.
    pub text: usize,
    /// The number of damaged custom primitives.
    pub primitives: usize,
    /// The number of damaged images.
    pub images: usize,
    /// Whether damage was produced from a detected scroll.
    pub scroll: bool,
}

impl Summary {
    /// Adds another [`Summary`] to this one.
    pub fn extend(&mut self, other: Self) {
        self.quads += other.quads;
        self.text += other.text;
        self.primitives += other.primitives;
        self.images += other.images;
        self.scroll |= other.scroll;
    }
}

/// A detected scroll region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scroll {
    /// The scrolled bounds.
    pub bounds: Rectangle,
    /// The scroll delta.
    pub delta: Vector,
}

impl Scroll {
    /// Returns the newly exposed damage region caused by the scroll.
    pub fn damage(self) -> Vec<Rectangle> {
        if self.delta.y < 0.0 {
            vec![Rectangle {
                x: self.bounds.x,
                y: self.bounds.y + self.bounds.height + self.delta.y,
                width: self.bounds.width,
                height: -self.delta.y,
            }]
        } else {
            vec![Rectangle {
                x: self.bounds.x,
                y: self.bounds.y,
                width: self.bounds.width,
                height: self.delta.y,
            }]
        }
    }
}

/// The strategy chosen to collapse fragmented damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Keep grouped damage as-is.
    Grouped,
    /// Collapse damage to a union region or full bounds.
    Union,
}

impl Strategy {
    /// Returns a stable label for tracing.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grouped => "grouped",
            Self::Union => "union",
        }
    }
}

/// Diffs the damage regions given some previous and current primitives.
pub fn diff<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    diff: impl Fn(&T, &T) -> Vec<Rectangle>,
) -> Vec<Rectangle> {
    let damage = previous.iter().zip(current).flat_map(|(a, b)| diff(a, b));

    if previous.len() == current.len() {
        damage.collect()
    } else {
        let (smaller, bigger) = if previous.len() < current.len() {
            (previous, current)
        } else {
            (current, previous)
        };

        // Extend damage by the added/removed primitives
        damage
            .chain(bigger[smaller.len()..].iter().flat_map(bounds))
            .collect()
    }
}

/// Computes the damage regions given some previous and current primitives.
pub fn list<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    are_equal: impl Fn(&T, &T) -> bool,
) -> Vec<Rectangle> {
    diff(previous, current, &bounds, |a, b| {
        if are_equal(a, b) {
            vec![]
        } else {
            bounds(a).into_iter().chain(bounds(b)).collect()
        }
    })
}

/// Groups the given damage regions that are close together inside the given
/// bounds.
pub fn group(mut damage: Vec<Rectangle>, bounds: Rectangle) -> Vec<Rectangle> {
    const AREA_THRESHOLD: f32 = 20_000.0;

    damage.sort_by(|a, b| {
        a.center()
            .distance(Point::ORIGIN)
            .total_cmp(&b.center().distance(Point::ORIGIN))
    });

    let mut output = Vec::new();
    let mut scaled = damage
        .into_iter()
        .filter_map(|region| region.intersection(&bounds))
        .filter(|region| region.width >= 1.0 && region.height >= 1.0);

    if let Some(mut current) = scaled.next() {
        for region in scaled {
            let union = current.union(&region);

            if union.area() - current.area() - region.area() <= AREA_THRESHOLD {
                current = union;
            } else {
                output.push(current);
                current = region;
            }
        }

        output.push(current);
    }

    output
}

/// Deduplicates compatible scroll hints and rejects conflicting overlaps.
pub fn collect_scrolls(scrolls: impl IntoIterator<Item = Scroll>) -> Vec<Scroll> {
    let mut accepted: Vec<Scroll> = Vec::new();

    for scroll in scrolls {
        let mut duplicate = false;

        for accepted_scroll in &accepted {
            if accepted_scroll.bounds == scroll.bounds {
                if accepted_scroll.delta == scroll.delta {
                    duplicate = true;
                    break;
                }

                return Vec::new();
            }

            if accepted_scroll
                .bounds
                .intersection(&scroll.bounds)
                .is_some()
            {
                return Vec::new();
            }
        }

        if !duplicate {
            accepted.push(scroll);
        }
    }

    accepted
}

/// Collapses fragmented damage when a union is cheaper than many small regions.
pub fn collapse_fragmented(
    damage: Vec<Rectangle>,
    bounds: Rectangle,
    summary: Summary,
) -> (Vec<Rectangle>, Strategy) {
    const REGION_THRESHOLD: usize = 8;
    const TEXT_REGION_THRESHOLD: usize = 2;
    const TEXT_DAMAGE_THRESHOLD: usize = 32;
    const TEXT_REGION_BUDGET: usize = 8;
    const VIEWPORT_AREA_THRESHOLD: f32 = 0.35;
    const UNION_WASTE_THRESHOLD: f32 = 2.0;

    let Some(union) = damage.iter().copied().reduce(|a, b| a.union(&b)) else {
        return (damage, Strategy::Grouped);
    };

    let damage_area = damage.iter().map(Rectangle::area).sum::<f32>();
    let bounds_area = bounds.area();
    let covers_large_viewport = damage_area >= bounds_area * VIEWPORT_AREA_THRESHOLD;
    let text_heavy_fragmentation =
        summary.text >= TEXT_DAMAGE_THRESHOLD && damage.len() >= TEXT_REGION_THRESHOLD;

    if covers_large_viewport {
        return (vec![bounds], Strategy::Union);
    }

    let union = union.intersection(&bounds).unwrap_or(bounds);
    let union_is_not_wasteful = union.area() <= damage_area * UNION_WASTE_THRESHOLD;

    if text_heavy_fragmentation && damage.len() > TEXT_REGION_BUDGET && !union_is_not_wasteful {
        return (
            compact_to_budget(damage, TEXT_REGION_BUDGET),
            Strategy::Grouped,
        );
    }

    if text_heavy_fragmentation && union_is_not_wasteful {
        return (vec![union], Strategy::Union);
    }

    if damage.len() < REGION_THRESHOLD {
        return (damage, Strategy::Grouped);
    }

    if union_is_not_wasteful {
        (vec![union], Strategy::Union)
    } else {
        (damage, Strategy::Grouped)
    }
}

fn compact_to_budget(mut damage: Vec<Rectangle>, budget: usize) -> Vec<Rectangle> {
    if budget == 0 || damage.len() <= budget {
        return damage;
    }
    // Bound pathological work. Redrawing the union preserves final pixels.
    if damage.len() > 256 {
        return damage
            .into_iter()
            .reduce(|a, b| a.union(&b))
            .into_iter()
            .collect();
    }

    // Cache the minimum for each left-hand row of the pair matrix. Merging
    // changes just two slots; retain unaffected minima and compare the new
    // slots, rescanning only rows whose previous winner became invalid.
    let mut minima = (0..damage.len() - 1)
        .map(|left| best_in_row(&damage, left))
        .collect::<Vec<_>>();
    while damage.len() > budget {
        let mut pair = minima[0];
        for &candidate in &minima[1..] {
            if candidate.better_than(pair) {
                pair = candidate;
            }
        }
        let (left, right) = (pair.left, pair.right);
        let merged = damage[left].union(&damage[right]);
        damage[left] = merged;
        let _ = damage.swap_remove(right);
        minima.truncate(damage.len() - 1);
        for (row, minimum) in minima.iter_mut().enumerate() {
            if row == left
                || row == right
                || minimum.right == left
                || minimum.right == right
                || minimum.right >= damage.len()
            {
                *minimum = best_in_row(&damage, row);
            } else {
                for changed in [left, right] {
                    if changed > row && changed < damage.len() {
                        let candidate = MergePair::new(&damage, row, changed);
                        if candidate.better_than(*minimum) {
                            *minimum = candidate;
                        }
                    }
                }
            }
        }
    }

    damage
}

fn best_in_row(damage: &[Rectangle], left: usize) -> MergePair {
    let mut best = MergePair::new(damage, left, left + 1);
    for right in left + 2..damage.len() {
        let candidate = MergePair::new(damage, left, right);
        if candidate.better_than(best) {
            best = candidate;
        }
    }
    best
}

#[derive(Debug, Clone, Copy)]
struct MergePair {
    extra_area: f32,
    union_area: f32,
    left: usize,
    right: usize,
}

impl MergePair {
    fn new(damage: &[Rectangle], left: usize, right: usize) -> Self {
        let union_area = damage[left].union(&damage[right]).area();
        Self {
            extra_area: union_area - damage[left].area() - damage[right].area(),
            union_area,
            left,
            right,
        }
    }
    fn better_than(self, other: Self) -> bool {
        self.extra_area < other.extra_area
            || (self.extra_area == other.extra_area
                && (self.union_area < other.union_area
                    || (self.union_area == other.union_area
                        && (self.left, self.right) < (other.left, other.right))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;

    fn reference(mut damage: Vec<Rectangle>, budget: usize) -> Vec<Rectangle> {
        while damage.len() > budget {
            let mut best = (0, 1);
            let mut extra = f32::INFINITY;
            let mut area = f32::INFINITY;
            for left in 0..damage.len() {
                for right in left + 1..damage.len() {
                    let a = damage[left].union(&damage[right]).area();
                    let e = a - damage[left].area() - damage[right].area();
                    if e < extra || (e == extra && a < area) {
                        best = (left, right);
                        extra = e;
                        area = a;
                    }
                }
            }
            damage[best.0] = damage[best.0].union(&damage[best.1]);
            let _ = damage.swap_remove(best.1);
        }
        damage
    }
    #[test]
    fn cached_compaction_matches_original_greedy_pair_order() {
        let mut state = 17u32;
        for count in [9, 16, 30, 60, 128, 256] {
            for _ in 0..20 {
                let damage = (0..count)
                    .map(|_| {
                        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                        Rectangle::new(
                            Point::new(
                                (state % 20) as f32 * 17.0,
                                ((state >> 9) % 20) as f32 * 19.0,
                            ),
                            Size::new(20.0, 18.0),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(compact_to_budget(damage.clone(), 8), reference(damage, 8));
            }
        }
    }

    #[test]
    #[ignore = "manual release microbenchmark"]
    fn benchmark_compaction_against_reference() {
        use std::hint::black_box;
        use std::time::Instant;
        for count in [16, 32, 64, 128, 256] {
            let damage = (0..count)
                .map(|i| {
                    Rectangle::new(
                        Point::new((i % 8) as f32 * 140.0, (i / 8) as f32 * 70.0),
                        Size::new(20.0 + (i % 3) as f32, 18.0),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                compact_to_budget(damage.clone(), 8),
                reference(damage.clone(), 8)
            );
            for (name, run) in [
                (
                    "original",
                    reference as fn(Vec<Rectangle>, usize) -> Vec<Rectangle>,
                ),
                ("cached", compact_to_budget),
            ] {
                let loops = 1000;
                let started = Instant::now();
                for _ in 0..loops {
                    let _ = black_box(run(black_box(damage.clone()), 8));
                }
                eprintln!(
                    "regions={count} algorithm={name} us={:.3}",
                    started.elapsed().as_secs_f64() * 1e6 / loops as f64
                );
            }
        }
    }

    #[test]
    fn extreme_fragmentation_retains_all_damage_without_quadratic_storage() {
        let damage = (0..4096)
            .map(|i| {
                Rectangle::new(
                    Point::new((i % 64) as f32 * 40.0, (i / 64) as f32 * 40.0),
                    Size::new(4.0, 4.0),
                )
            })
            .collect::<Vec<_>>();
        let compacted = compact_to_budget(damage.clone(), 8);
        assert_eq!(compacted.len(), 1);
        assert!(damage.iter().all(|rect| rect.is_within(&compacted[0])));
    }

    fn scroll(x: f32, y: f32, width: f32, height: f32, delta_y: f32) -> Scroll {
        Scroll {
            bounds: Rectangle {
                x,
                y,
                width,
                height,
            },
            delta: Vector::new(0.0, delta_y),
        }
    }

    #[test]
    fn collect_scrolls_deduplicates_identical_layer_hints() {
        let hint = scroll(10.0, 20.0, 300.0, 400.0, -18.0);

        assert_eq!(collect_scrolls([hint, hint]), vec![hint]);
    }

    #[test]
    fn collect_scrolls_rejects_overlapping_conflicting_hints() {
        let first = scroll(10.0, 20.0, 300.0, 400.0, -18.0);
        let second = scroll(10.0, 20.0, 300.0, 400.0, 18.0);

        assert!(collect_scrolls([first, second]).is_empty());
    }

    #[test]
    fn collect_scrolls_accepts_disjoint_hints() {
        let first = scroll(0.0, 0.0, 100.0, 100.0, -18.0);
        let second = scroll(120.0, 0.0, 100.0, 100.0, -18.0);

        assert_eq!(collect_scrolls([first, second]), vec![first, second]);
    }

    #[test]
    fn fragmented_damage_collapses_to_union_when_it_covers_much_of_viewport() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
        let damage = (0..20)
            .map(|index| Rectangle {
                x: (index % 5) as f32 * 20.0,
                y: (index / 5) as f32 * 20.0,
                width: 18.0,
                height: 18.0,
            })
            .collect();

        let (damage, strategy) = collapse_fragmented(damage, bounds, Summary::default());

        assert_eq!(strategy, Strategy::Union);
        assert_eq!(damage.len(), 1);
    }

    #[test]
    fn medium_fragmented_large_area_damage_collapses_to_union() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1_024.0, 768.0));
        let damage = (0..15)
            .map(|index| Rectangle {
                x: 0.0,
                y: index as f32 * 35.0,
                width: 1_024.0,
                height: 28.0,
            })
            .collect();

        let (damage, strategy) = collapse_fragmented(damage, bounds, Summary::default());

        assert_eq!(strategy, Strategy::Union);
        assert_eq!(damage.len(), 1);
    }

    #[test]
    fn low_count_large_area_damage_collapses_to_full_bounds() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1_024.0, 768.0));
        let damage = (0..6)
            .map(|index| Rectangle {
                x: 0.0,
                y: index as f32 * 90.0,
                width: 1_024.0,
                height: 75.0,
            })
            .collect();

        let (damage, strategy) = collapse_fragmented(damage, bounds, Summary::default());

        assert_eq!(strategy, Strategy::Union);
        assert_eq!(damage, vec![bounds]);
    }

    #[test]
    fn text_heavy_fragmented_damage_is_compacted_when_union_is_wasteful() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1_024.0, 768.0));
        let damage = (0..20)
            .map(|index| Rectangle {
                x: 0.0,
                y: index as f32 * 42.0,
                width: 120.0,
                height: 18.0,
            })
            .collect();
        let summary = Summary {
            text: 88,
            ..Summary::default()
        };

        let (damage, strategy) = collapse_fragmented(damage, bounds, summary);

        assert_eq!(strategy, Strategy::Grouped);
        assert_eq!(damage.len(), 8);
        assert!(damage.iter().map(Rectangle::area).sum::<f32>() < bounds.area());
    }

    #[test]
    fn text_heavy_two_region_damage_stays_grouped_when_union_is_wasteful() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1_024.0, 768.0));
        let damage = vec![
            Rectangle {
                x: 0.0,
                y: 40.0,
                width: 220.0,
                height: 18.0,
            },
            Rectangle {
                x: 0.0,
                y: 220.0,
                width: 220.0,
                height: 18.0,
            },
        ];
        let summary = Summary {
            text: 88,
            ..Summary::default()
        };

        let (damage, strategy) = collapse_fragmented(damage, bounds, summary);

        assert_eq!(strategy, Strategy::Grouped);
        assert_eq!(damage.len(), 2);
    }

    #[test]
    fn text_heavy_fragmented_damage_is_compacted_when_region_count_is_excessive() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1_024.0, 768.0));
        let damage = (0..40)
            .map(|index| Rectangle {
                x: 0.0,
                y: index as f32 * 18.0,
                width: 220.0,
                height: 8.0,
            })
            .collect();
        let summary = Summary {
            text: 88,
            ..Summary::default()
        };

        let (damage, strategy) = collapse_fragmented(damage, bounds, summary);

        assert_eq!(strategy, Strategy::Grouped);
        assert_eq!(damage.len(), 8);
    }

    #[test]
    fn text_heavy_fragmented_damage_collapses_when_union_is_compact() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1_024.0, 768.0));
        let damage = (0..12)
            .map(|index| Rectangle {
                x: 0.0,
                y: index as f32 * 20.0,
                width: 220.0,
                height: 18.0,
            })
            .collect();
        let summary = Summary {
            text: 88,
            ..Summary::default()
        };

        let (damage, strategy) = collapse_fragmented(damage, bounds, summary);

        assert_eq!(strategy, Strategy::Union);
        assert_eq!(damage.len(), 1);
    }

    #[test]
    fn sparse_fragmented_damage_stays_grouped_when_union_is_wasteful() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1_000.0, 1_000.0));
        let damage = (0..16)
            .map(|index| Rectangle {
                x: index as f32 * 60.0,
                y: index as f32 * 60.0,
                width: 4.0,
                height: 4.0,
            })
            .collect();

        let (damage, strategy) = collapse_fragmented(damage, bounds, Summary::default());

        assert_eq!(strategy, Strategy::Grouped);
        assert_eq!(damage.len(), 16);
    }

    #[test]
    fn low_region_count_damage_stays_grouped() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(100.0, 100.0));
        let damage = vec![Rectangle {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        }];

        let (damage, strategy) = collapse_fragmented(damage, bounds, Summary::default());

        assert_eq!(strategy, Strategy::Grouped);
        assert_eq!(damage.len(), 1);
    }
}
