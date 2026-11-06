use super::{Anchor, BLACK, Mark, TEXT, text_width};
use anyhow::{Result, bail};

pub type Rect = (f64, f64, f64, f64);

#[derive(Clone, Copy, Debug)]
pub struct Anchor2 {
    pub at: (f64, f64),
    pub radius: f64,
}

impl Anchor2 {
    fn toward(&self, to: (f64, f64)) -> (f64, f64) {
        if self.radius <= 0.0 {
            return self.at;
        }
        let (dx, dy) = (to.0 - self.at.0, to.1 - self.at.1);
        let n = dx.hypot(dy).max(1.0e-9);
        (
            self.at.0 + dx / n * self.radius,
            self.at.1 + dy / n * self.radius,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Label {
    pub text: String,
    pub anchors: Vec<Anchor2>,
    pub side: Option<Side>,
    pub single: bool,
}

pub fn parse_side(text: &str) -> Result<Side> {
    Ok(match text {
        "left" => Side::Left,
        "right" => Side::Right,
        "top" => Side::Top,
        "bottom" => Side::Bottom,
        other => bail!("`side` is left, right, top or bottom, not `{other}`"),
    })
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    fn opposite(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
        }
    }

    fn vertical_column(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
}

pub fn overlaps(a: Rect, b: Rect) -> bool {
    a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1
}

pub fn text_rect(x: f64, y: f64, size: f64, text: &str, anchor: Anchor, angle: f64) -> Rect {
    let w = text_width(text, size);
    let along = match anchor {
        Anchor::Start => (0.0, w),
        Anchor::Middle => (-w / 2.0, w / 2.0),
        Anchor::End => (-w, 0.0),
    };
    if angle != 0.0 {
        (x - size, y - along.1, x + 0.3, y - along.0)
    } else {
        (x + along.0, y - size, x + along.1, y + 0.3)
    }
}

pub fn mark_rects(mark: &Mark) -> Vec<Rect> {
    match mark {
        Mark::Text {
            x,
            y,
            size,
            text,
            anchor,
            angle,
        } => vec![text_rect(*x, *y, *size, text, *anchor, *angle)],
        Mark::Shape {
            loops,
            fill: Some(_),
            ..
        } => {
            let points = loops.iter().flatten();
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in points {
                x0 = x0.min(p.0);
                y0 = y0.min(p.1);
                x1 = x1.max(p.0);
                y1 = y1.max(p.1);
            }
            if x0 > x1 {
                Vec::new()
            } else {
                vec![(x0, y0, x1, y1)]
            }
        }
        Mark::Shape { loops, .. } => loops
            .iter()
            .flat_map(|l| {
                l.windows(2).map(|w| {
                    (
                        w[0].0.min(w[1].0) - 0.4,
                        w[0].1.min(w[1].1) - 0.4,
                        w[0].0.max(w[1].0) + 0.4,
                        w[0].1.max(w[1].1) + 0.4,
                    )
                })
            })
            .collect(),
    }
}

fn spread(desired: &[f64], extent: &[f64], gap: f64, low: f64, high: f64) -> Vec<f64> {
    let mut at = desired.to_vec();
    for _ in 0..400 {
        let mut moved = false;
        for i in 1..at.len() {
            let need = (extent[i - 1] + extent[i]) / 2.0 + gap;
            let over = need - (at[i] - at[i - 1]);
            if over > 1.0e-6 {
                at[i - 1] -= over / 2.0;
                at[i] += over / 2.0;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    if let (Some(first), Some(last)) = (at.first().copied(), at.last().copied()) {
        let first_low = first - extent[0] / 2.0;
        let last_high = last + extent[extent.len() - 1] / 2.0;
        let shift = if first_low < low {
            low - first_low
        } else if last_high > high {
            high - last_high
        } else {
            0.0
        };
        at.iter_mut().for_each(|v| *v += shift);
    }
    at
}

fn centre(label: &Label) -> (f64, f64) {
    let n = label.anchors.len().max(1) as f64;
    (
        label.anchors.iter().map(|a| a.at.0).sum::<f64>() / n,
        label.anchors.iter().map(|a| a.at.1).sum::<f64>() / n,
    )
}

fn preferred(view: Rect, label: &Label) -> Side {
    if let Some(side) = label.side {
        return side;
    }
    let (cx, _) = centre(label);
    if (cx - view.0) + 0.05 * (view.2 - view.0) < view.2 - cx {
        Side::Left
    } else {
        Side::Right
    }
}

struct Page {
    width: f64,
    height: f64,
    margin: f64,
}

impl Page {
    fn holds(&self, r: Rect) -> bool {
        r.0 >= self.margin + 1.0
            && r.1 >= self.margin + 1.0
            && r.2 <= self.width - self.margin - 1.0
            && r.3 <= self.height - self.margin - 1.0
    }
}

fn crosses(a: (f64, f64), b: (f64, f64), r: Rect) -> bool {
    let steps = (((b.0 - a.0).hypot(b.1 - a.1)) / 0.5).ceil().max(1.0) as usize;
    (1..steps).any(|i| {
        let t = i as f64 / steps as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        x > r.0 && x < r.2 && y > r.1 && y < r.3
    })
}

struct Drawn {
    dots: Vec<(f64, f64)>,
    leaders: Vec<Vec<(f64, f64)>>,
    texts: Vec<Mark>,
    rects: Vec<Rect>,
}

fn column(
    side: Side,
    labels: &[&Label],
    view: Rect,
    page: &Page,
    taken: &[Rect],
    texts: &[Rect],
    blocks: &[Rect],
) -> Option<Drawn> {
    let mut order: Vec<&Label> = labels.to_vec();
    let key = |l: &Label| {
        let c = centre(l);
        if side.vertical_column() { c.1 } else { c.0 }
    };
    order.sort_by(|a, b| key(a).total_cmp(&key(b)));
    let desired: Vec<f64> = order.iter().map(|l| key(l)).collect();
    let widths: Vec<f64> = order
        .iter()
        .map(|l| text_width(&l.text, TEXT) + 2.0)
        .collect();
    let slots = if side.vertical_column() {
        let extent = vec![TEXT * 1.6 + 1.0; order.len()];
        spread(
            &desired,
            &extent,
            0.0,
            page.margin + 1.0 + TEXT,
            page.height - page.margin - 1.0,
        )
    } else {
        spread(
            &desired,
            &widths,
            2.0,
            page.margin + 1.0,
            page.width - page.margin - 1.0,
        )
    };
    for step in 0..30 {
        let d = 5.0 + 3.0 * step as f64;
        let mut drawn = Drawn {
            dots: Vec::new(),
            leaders: Vec::new(),
            texts: Vec::new(),
            rects: Vec::new(),
        };
        for ((label, &slot), &w) in order.iter().zip(&slots).zip(&widths) {
            let (join, shelf, text_at, anchor, rect) = match side {
                Side::Right => {
                    let gx = view.2 + d;
                    (
                        (gx, slot),
                        vec![(gx, slot), (gx + w, slot)],
                        (gx + 0.5, slot - 0.8),
                        Anchor::Start,
                        (gx, slot - TEXT - 0.5, gx + w, slot + 0.3),
                    )
                }
                Side::Left => {
                    let gx = view.0 - d;
                    (
                        (gx, slot),
                        vec![(gx, slot), (gx - w, slot)],
                        (gx - 0.5, slot - 0.8),
                        Anchor::End,
                        (gx - w, slot - TEXT - 0.5, gx, slot + 0.3),
                    )
                }
                Side::Top => {
                    let gy = view.1 - d;
                    (
                        (slot, gy),
                        vec![(slot - w / 2.0, gy), (slot + w / 2.0, gy)],
                        (slot, gy - 0.8),
                        Anchor::Middle,
                        (slot - w / 2.0, gy - TEXT - 0.5, slot + w / 2.0, gy + 0.3),
                    )
                }
                Side::Bottom => {
                    let gy = view.3 + d;
                    (
                        (slot, gy),
                        vec![(slot - w / 2.0, gy), (slot + w / 2.0, gy)],
                        (slot, gy + TEXT + 0.2),
                        Anchor::Middle,
                        (slot - w / 2.0, gy - 0.3, slot + w / 2.0, gy + TEXT + 1.0),
                    )
                }
            };
            drawn.leaders.push(shelf);
            let nearest = |a: &&Anchor2| {
                let e = a.toward(join);
                (e.0 - join.0).hypot(e.1 - join.1)
            };
            let chosen: Vec<&Anchor2> = if label.single {
                label
                    .anchors
                    .iter()
                    .min_by(|a, b| nearest(a).total_cmp(&nearest(b)))
                    .into_iter()
                    .collect()
            } else {
                label.anchors.iter().collect()
            };
            for a in chosen {
                let end = a.toward(join);
                drawn.leaders.push(vec![end, join]);
                drawn.dots.push(end);
            }
            drawn.texts.push(Mark::Text {
                x: text_at.0,
                y: text_at.1,
                size: TEXT,
                text: label.text.clone(),
                anchor,
                angle: 0.0,
            });
            drawn.rects.push(rect);
        }
        let free = drawn
            .rects
            .iter()
            .all(|r| page.holds(*r) && taken.iter().all(|t| !overlaps(*r, *t)))
            && drawn.leaders.iter().all(|l| {
                l.windows(2)
                    .all(|w| texts.iter().chain(blocks).all(|t| !crosses(w[0], w[1], *t)))
            });
        if free {
            return Some(drawn);
        }
        let off_page = drawn.rects.iter().any(|r| {
            r.0 < page.margin
                || r.2 > page.width - page.margin
                || r.1 < page.margin
                || r.3 > page.height - page.margin
        });
        if off_page && step > 2 {
            return None;
        }
    }
    None
}

fn cost(drawn: &Drawn) -> f64 {
    drawn
        .leaders
        .iter()
        .map(|l| {
            l.windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum::<f64>()
        })
        .sum()
}

fn place(
    side: Side,
    group: &[&Label],
    view: Rect,
    page: &Page,
    taken: &mut Vec<Rect>,
    texts: &[Rect],
    blocks: &[Rect],
) -> Option<Vec<Drawn>> {
    let order = match side {
        Side::Left | Side::Right => [side, side.opposite(), Side::Bottom, Side::Top],
        Side::Top | Side::Bottom => [side, side.opposite(), Side::Right, Side::Left],
    };
    let tried: Vec<Drawn> = order[..2]
        .iter()
        .filter_map(|&s| column(s, group, view, page, taken, texts, blocks))
        .collect();
    let best = tried
        .into_iter()
        .min_by(|a, b| cost(a).total_cmp(&cost(b)))
        .or_else(|| {
            order[2..]
                .iter()
                .find_map(|&s| column(s, group, view, page, taken, texts, blocks))
        });
    if let Some(drawn) = best {
        taken.extend(drawn.rects.iter().copied());
        return Some(vec![drawn]);
    }
    if group.len() < 2 {
        return None;
    }
    let mut sorted = group.to_vec();
    let key = |l: &Label| {
        let c = centre(l);
        if side.vertical_column() { c.1 } else { c.0 }
    };
    sorted.sort_by(|a, b| key(a).total_cmp(&key(b)));
    let (first, second) = sorted.split_at(sorted.len() / 2);
    let mut out = place(side, first, view, page, taken, texts, blocks)?;
    out.extend(place(
        side.opposite(),
        second,
        view,
        page,
        taken,
        texts,
        blocks,
    )?);
    Some(out)
}

pub fn layout(
    view: Rect,
    page: (f64, f64, f64),
    obstacles: &[Rect],
    texts: &[Rect],
    blocks: &[Rect],
    labels: &[Label],
) -> Result<Vec<Mark>> {
    let page = Page {
        width: page.0,
        height: page.1,
        margin: page.2,
    };
    let mut taken: Vec<Rect> = obstacles.to_vec();
    let mut marks = Vec::new();
    let mut leaders: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut dots: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut ends: Vec<(f64, f64)> = Vec::new();
    for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
        let group: Vec<&Label> = labels
            .iter()
            .filter(|l| preferred(view, l) == side)
            .collect();
        if group.is_empty() {
            continue;
        }
        let drawn = place(side, &group, view, &page, &mut taken, texts, blocks).ok_or_else(|| {
            anyhow::anyhow!(
                "there is no room beside the view for {} (a smaller `scale`, a `window`, or a bigger sheet)",
                group
                    .iter()
                    .map(|l| format!("`{}`", l.text))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        for d in drawn {
            leaders.extend(d.leaders);
            ends.extend(d.dots);
            marks.extend(d.texts);
        }
    }
    for a in &ends {
        dots.push(
            (0..8)
                .map(|i| {
                    let t = i as f64 * std::f64::consts::PI / 4.0;
                    (a.0 + 0.5 * t.cos(), a.1 + 0.5 * t.sin())
                })
                .collect(),
        );
    }
    let mut out = vec![
        Mark::Shape {
            loops: leaders,
            closed: false,
            fill: None,
            stroke: Some(BLACK),
            width: 0.13,
            dashed: false,
        },
        Mark::Shape {
            loops: dots,
            closed: true,
            fill: Some(BLACK),
            stroke: None,
            width: 0.0,
            dashed: false,
        },
    ];
    out.extend(marks);
    Ok(out)
}
