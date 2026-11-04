//! Panel nesting: packs flat parts onto stock sheets with guillotine cuts, so
//! every cut runs edge to edge across the piece of board it is made in, the
//! way a panel saw or a track saw works.

use crate::geometry;
use crate::model::Model;
use anyhow::{Result, bail};

const EPS: f64 = 1.0e-6;

/// A flat part to cut: its two face sizes (`length` the longer) and thickness.
#[derive(Clone, Debug)]
pub struct Piece {
    pub name: String,
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    /// One long side shows, so it must lie on a finished (edged) edge of the stock.
    pub edge: bool,
}

/// A piece placed on a stock sheet. `x` runs along the stock's length, `y`
/// across its width, from the corner at 0,0; `dx` and `dy` are the piece's
/// size along them.
#[derive(Clone, Debug)]
pub struct Placed {
    pub piece: usize,
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    /// The piece's long side lies on a finished edge of the stock.
    pub on_edge: bool,
}

/// One saw cut on a stock sheet. A rip runs along the stock's length at
/// `y = at`, from `x = from` to `to`; a crosscut runs across it at `x = at`,
/// from `y = from` to `to`. `at` is the edge of a piece; the kerf goes into
/// the waste side of it.
#[derive(Clone, Debug)]
pub struct Cut {
    pub rip: bool,
    pub at: f64,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Stock {
    pub length: f64,
    pub width: f64,
    pub placed: Vec<Placed>,
    /// In an order they can be sawn in: each cut lies inside a piece of board
    /// that earlier cuts have already freed.
    pub cuts: Vec<Cut>,
    /// The offcuts left over, each a rectangle of whole board.
    pub free: Vec<Rect>,
}

impl Stock {
    pub fn used(&self) -> f64 {
        self.placed.iter().map(|p| p.dx * p.dy).sum()
    }

    pub fn area(&self) -> f64 {
        self.length * self.width
    }
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub gap: f64,
    pub trim: f64,
    pub edged: bool,
    pub pieces: Vec<Piece>,
    pub sheets: Vec<Stock>,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// The stock sizes on offer, length and width; the layout buys whichever
    /// mix of them wastes the least board.
    pub stocks: Vec<(f64, f64)>,
    /// The least space between pieces: the saw kerf, plus any allowance.
    pub gap: f64,
    /// Trimmed off the stock's edges before cutting (chipped factory edges);
    /// on edged stock only off the two bare ends.
    pub trim: f64,
    /// Keep each piece's length along the stock's length (grain or pattern).
    pub grain: bool,
    /// The stock's two long edges are finished (edge banded at the factory).
    pub edged: bool,
}

/// `2440x1220,2440x600`: one or more stock sizes.
pub fn stock_sizes(text: &str) -> Result<Vec<(f64, f64)>> {
    text.trim_matches('"')
        .split([',', ' '])
        .filter(|t| !t.is_empty())
        .map(stock_size)
        .collect()
}

/// Marks the pieces whose names match the patterns as showing a long edge.
pub fn mark_edges(pieces: &mut [Piece], patterns: &[String]) -> Result<()> {
    for pattern in patterns {
        let mut hit = false;
        for p in pieces.iter_mut() {
            if crate::sim::matches(pattern, &p.name) {
                p.edge = true;
                hit = true;
            }
        }
        if !hit {
            bail!("no part to nest matches `{pattern}` in `edge`");
        }
    }
    Ok(())
}

/// `2400x600` as length and width, the longer first.
pub fn stock_size(text: &str) -> Result<(f64, f64)> {
    let parts: Vec<&str> = text.trim_matches('"').split(['x', 'X']).collect();
    let [a, b] = parts.as_slice() else {
        bail!("write the stock size as length x width, like 2400x600, not `{text}`");
    };
    let (a, b): (f64, f64) = (a.trim().parse()?, b.trim().parse()?);
    if a <= 0.0 || b <= 0.0 {
        bail!("a stock size is two positive numbers, like 2400x600");
    }
    Ok((a.max(b), a.min(b)))
}

/// The model's part names matching the patterns (`*` wildcards, `!` leaves out);
/// no wanted pattern means every part.
pub fn pick(model: &Model, patterns: &[String]) -> Result<Vec<String>> {
    let names: Vec<String> = model
        .named_solids()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| {
            let mut wanted = patterns.iter().filter(|p| !p.starts_with('!')).peekable();
            let keep = wanted.peek().is_none() || wanted.any(|p| crate::sim::matches(p, name));
            let drop = patterns
                .iter()
                .filter_map(|p| p.strip_prefix('!'))
                .any(|p| crate::sim::matches(p, name));
            keep && !drop
        })
        .collect();
    if names.is_empty() {
        bail!("no part matches {}", patterns.join(" "));
    }
    Ok(names)
}

/// The named parts as flat pieces, sized by their bounding box along the model
/// axes: the thinnest side is the thickness. Only parts that thick are kept
/// when `thickness` is given; otherwise every part must be the same thickness.
pub fn panels(model: &Model, names: &[String], thickness: Option<f64>) -> Result<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut found: Vec<f64> = Vec::new();
    for (name, solid) in model.named_solids() {
        if !names.contains(&name) {
            continue;
        }
        let b = geometry::bounds(solid);
        let mut size = [
            b.max().x - b.min().x,
            b.max().y - b.min().y,
            b.max().z - b.min().z,
        ];
        size.sort_by(f64::total_cmp);
        let t = round(size[0]);
        if thickness.is_some_and(|want| (want - t).abs() > 0.01) {
            continue;
        }
        if !found.iter().any(|f| (f - t).abs() < 0.01) {
            found.push(t);
        }
        pieces.push(Piece {
            name,
            length: round(size[2]),
            width: round(size[1]),
            thickness: t,
            edge: false,
        });
    }
    if pieces.is_empty() {
        match thickness {
            Some(t) => bail!("none of the parts is {} thick", number(t)),
            None => bail!("no parts to nest"),
        }
    }
    if found.len() > 1 {
        found.sort_by(f64::total_cmp);
        let list: Vec<String> = found.iter().map(|t| number(*t)).collect();
        bail!(
            "the parts are {} thick; pick one board with `thickness=`, or leave parts out by name",
            list.join(", ")
        );
    }
    Ok(pieces)
}

fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub fn number(value: f64) -> String {
    let text = format!("{value:.1}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Clone, Copy)]
enum Split {
    Shorter,
    Longer,
    RipFirst,
    CrossFirst,
}

#[derive(Clone, Copy)]
enum Fit {
    ShortSide,
    Area,
}

/// Packs the pieces onto stock, trying several orders, ways to split the
/// board and mixes of the stock sizes, and keeps the layout that buys the
/// least board, then the fewest sheets, the biggest single offcut and the
/// fewest cuts.
pub fn nest(pieces: Vec<Piece>, options: Options) -> Result<Layout> {
    let o = &options;
    if o.gap < 0.0 || o.trim < 0.0 {
        bail!("`gap` and `trim` cannot be negative");
    }
    if o.stocks.is_empty() {
        bail!("give at least one stock size");
    }
    if pieces.iter().any(|p| p.edge) && !o.edged {
        bail!(
            "`edge` parts need edged stock: say the stock's long edges are finished with `edged=1`"
        );
    }
    for p in &pieces {
        if !o.stocks.iter().any(|&s| fits_stock(p, s, o)) {
            let sizes: Vec<String> = o
                .stocks
                .iter()
                .map(|(l, w)| format!("{} x {}", number(*l), number(*w)))
                .collect();
            bail!(
                "{} ({} x {}) does not fit the {} stock{}",
                p.name,
                number(p.length),
                number(p.width),
                sizes.join(" or "),
                if o.trim > 0.0 { " after trimming" } else { "" }
            );
        }
    }
    let n = pieces.len();
    let keys: [fn(&Piece) -> (f64, f64); 5] = [
        |p| (p.length * p.width, p.length),
        |p| (p.length, p.width),
        |p| (p.width, p.length),
        |p| (p.length + p.width, p.length),
        |p| (p.length.max(p.width) - p.length.min(p.width), p.length),
    ];
    let orders: Vec<Vec<usize>> = keys
        .iter()
        .map(|key| {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| {
                let (ka, kb) = (key(&pieces[a]), key(&pieces[b]));
                pieces[b]
                    .edge
                    .cmp(&pieces[a].edge)
                    .then(kb.0.total_cmp(&ka.0))
                    .then(kb.1.total_cmp(&ka.1))
                    .then(a.cmp(&b))
            });
            order
        })
        .collect();
    let mut best: Option<(Vec<Stock>, (f64, usize, f64, usize))> = None;
    let mut try_all =
        |sequence: &[usize], best: &mut Option<(Vec<Stock>, (f64, usize, f64, usize))>| {
            for order in &orders {
                for split in [
                    Split::Shorter,
                    Split::Longer,
                    Split::RipFirst,
                    Split::CrossFirst,
                ] {
                    for fit in [Fit::ShortSide, Fit::Area] {
                        let sheets = pack(&pieces, order, o, split, fit, sequence);
                        let bought: f64 = sheets.iter().map(Stock::area).sum();
                        let biggest = sheets
                            .iter()
                            .flat_map(|s| &s.free)
                            .map(|r| r.w * r.h)
                            .fold(0.0, f64::max);
                        let cuts: usize = sheets.iter().map(|s| s.cuts.len()).sum();
                        let score = (bought, sheets.len(), -biggest, cuts);
                        let better = best.as_ref().is_none_or(|(_, b)| {
                            score.0.total_cmp(&(b.0 - 1.0)).is_lt()
                                || ((score.0 - b.0).abs() <= 1.0
                                    && score
                                        .1
                                        .cmp(&b.1)
                                        .then(score.2.total_cmp(&b.2))
                                        .then(score.3.cmp(&b.3))
                                        .is_lt())
                        });
                        if better {
                            *best = Some((sheets, score));
                        }
                    }
                }
            }
        };
    let k = o.stocks.len();
    for t in 0..k {
        try_all(&[t], &mut best);
    }
    if k > 1 {
        let most = best.as_ref().map_or(1, |(s, _)| s.len()).max(
            (0..k)
                .map(|t| {
                    let mut b = None;
                    try_all(&[t], &mut b);
                    b.map_or(1, |(s, _)| s.len())
                })
                .max()
                .unwrap_or(1),
        );
        let mut length = most.min(8);
        while (k as f64).powi(length as i32) > 4096.0 {
            length -= 1;
        }
        let mut sequence = vec![0; length];
        for code in 0..k.pow(length as u32) {
            let mut c = code;
            for slot in sequence.iter_mut() {
                *slot = c % k;
                c /= k;
            }
            try_all(&sequence, &mut best);
        }
    }
    let sheets = best.map(|(s, _)| s).unwrap_or_default();
    Ok(Layout {
        gap: o.gap,
        trim: o.trim,
        edged: o.edged,
        pieces,
        sheets,
    })
}

/// The board left to cut on a fresh stock sheet.
fn usable(stock: (f64, f64), o: &Options) -> Rect {
    let side = if o.edged { 0.0 } else { o.trim };
    Rect {
        x: o.trim,
        y: side,
        w: stock.0 - 2.0 * o.trim,
        h: stock.1 - 2.0 * side,
    }
}

fn turns(p: &Piece, o: &Options) -> Vec<(f64, f64)> {
    let mut turns = vec![(p.length, p.width)];
    if !o.grain && !p.edge && (p.length - p.width).abs() > EPS {
        turns.push((p.width, p.length));
    }
    turns
}

fn fits_stock(p: &Piece, stock: (f64, f64), o: &Options) -> bool {
    let r = usable(stock, o);
    turns(p, o)
        .iter()
        .any(|&(dx, dy)| dx <= r.w + EPS && dy <= r.h + EPS)
}

fn pack(
    pieces: &[Piece],
    order: &[usize],
    o: &Options,
    split: Split,
    fit: Fit,
    sequence: &[usize],
) -> Vec<Stock> {
    let mut sheets: Vec<Stock> = Vec::new();
    for &i in order {
        let p = &pieces[i];
        let turns = turns(p, o);
        let mut choice: Option<(f64, f64, usize, usize, f64, f64, bool)> = None;
        for (s, sheet) in sheets.iter().enumerate() {
            for (r, rect) in sheet.free.iter().enumerate() {
                let low = rect.y <= EPS;
                let high = rect.y + rect.h >= sheet.width - EPS;
                if p.edge && !low && !high {
                    continue;
                }
                for &(dx, dy) in &turns {
                    if dx > rect.w + EPS || dy > rect.h + EPS {
                        continue;
                    }
                    let (lw, lh) = (rect.w - dx, rect.h - dy);
                    let score = match fit {
                        Fit::ShortSide => (lw.min(lh), lw.max(lh)),
                        Fit::Area => (rect.w * rect.h - dx * dy, lw.min(lh)),
                    };
                    if choice.is_none_or(|c| {
                        score.0 < c.0 - EPS || (score.0 < c.0 + EPS && score.1 < c.1)
                    }) {
                        choice = Some((score.0, score.1, s, r, dx, dy, p.edge && !low));
                    }
                }
            }
        }
        let (s, r, dx, dy, top) = match choice {
            Some((_, _, s, r, dx, dy, top)) => (s, r, dx, dy, top),
            None => {
                let wanted = sequence
                    .get(sheets.len())
                    .or(sequence.last())
                    .copied()
                    .unwrap_or(0);
                let t = if fits_stock(p, o.stocks[wanted], o) {
                    wanted
                } else {
                    (0..o.stocks.len())
                        .filter(|&t| fits_stock(p, o.stocks[t], o))
                        .min_by(|&a, &b| {
                            let area = |t: usize| o.stocks[t].0 * o.stocks[t].1;
                            area(a).total_cmp(&area(b))
                        })
                        .expect("checked that every piece fits some stock")
                };
                let rect = usable(o.stocks[t], o);
                sheets.push(Stock {
                    length: o.stocks[t].0,
                    width: o.stocks[t].1,
                    free: vec![rect],
                    ..Stock::default()
                });
                let &(dx, dy) = turns
                    .iter()
                    .find(|(dx, dy)| *dx <= rect.w + EPS && *dy <= rect.h + EPS)
                    .expect("checked that the piece fits this stock");
                (sheets.len() - 1, 0, dx, dy, false)
            }
        };
        let sheet = &mut sheets[s];
        let rect = sheet.free.remove(r);
        place(sheet, rect, i, dx, dy, o.gap, split, top, p.edge);
    }
    sheets
}

/// Puts the piece in a corner of the free rectangle, at its bottom (low `y`)
/// or, with `top`, against its top, and saws the rest into new free
/// rectangles with one rip and one crosscut.
#[allow(clippy::too_many_arguments)]
fn place(
    sheet: &mut Stock,
    rect: Rect,
    piece: usize,
    dx: f64,
    dy: f64,
    gap: f64,
    split: Split,
    top: bool,
    edge: bool,
) {
    let (lw, lh) = (rect.w - dx, rect.h - dy);
    let py = if top { rect.y + lh } else { rect.y };
    sheet.placed.push(Placed {
        piece,
        x: rect.x,
        y: py,
        dx,
        dy,
        on_edge: edge,
    });
    let rip_first = match split {
        Split::Shorter => lw > lh,
        Split::Longer => lw <= lh,
        Split::RipFirst => true,
        Split::CrossFirst => false,
    };
    let rip_at = if top { py } else { py + dy };
    let rest_y = if top { rect.y } else { py + dy + gap };
    let mut keep = |r: Rect| {
        if r.w > EPS && r.h > EPS {
            sheet.free.push(r);
        }
    };
    let mut cuts = Vec::new();
    if rip_first {
        if lh > EPS {
            cuts.push(Cut {
                rip: true,
                at: rip_at,
                from: rect.x,
                to: rect.x + rect.w,
            });
            keep(Rect {
                x: rect.x,
                y: rest_y,
                w: rect.w,
                h: lh - gap,
            });
        }
        if lw > EPS {
            cuts.push(Cut {
                rip: false,
                at: rect.x + dx,
                from: py,
                to: py + dy,
            });
            keep(Rect {
                x: rect.x + dx + gap,
                y: py,
                w: lw - gap,
                h: dy,
            });
        }
    } else {
        if lw > EPS {
            cuts.push(Cut {
                rip: false,
                at: rect.x + dx,
                from: rect.y,
                to: rect.y + rect.h,
            });
            keep(Rect {
                x: rect.x + dx + gap,
                y: rect.y,
                w: lw - gap,
                h: rect.h,
            });
        }
        if lh > EPS {
            cuts.push(Cut {
                rip: true,
                at: rip_at,
                from: rect.x,
                to: rect.x + dx,
            });
            keep(Rect {
                x: rect.x,
                y: rest_y,
                w: dx,
                h: lh - gap,
            });
        }
    }
    sheet.cuts.extend(cuts);
}

impl Layout {
    /// The board bought, the parts cut from it and the rest, all in mm².
    pub fn areas(&self) -> (f64, f64, f64) {
        let bought: f64 = self.sheets.iter().map(Stock::area).sum();
        let parts: f64 = self.sheets.iter().map(Stock::used).sum();
        (bought, parts, bought - parts)
    }

    /// The stock bought, as `1 x 2440 x 1220 + 1 x 2440 x 600`.
    pub fn bought(&self) -> String {
        let mut sizes: Vec<((f64, f64), usize)> = Vec::new();
        for s in &self.sheets {
            match sizes
                .iter_mut()
                .find(|(z, _)| (z.0 - s.length).abs() < EPS && (z.1 - s.width).abs() < EPS)
            {
                Some((_, n)) => *n += 1,
                None => sizes.push(((s.length, s.width), 1)),
            }
        }
        sizes
            .iter()
            .map(|((l, w), n)| format!("{n} x {} x {}", number(*l), number(*w)))
            .collect::<Vec<_>>()
            .join(" + ")
    }

    /// One line: what is bought and how much of it is wasted.
    pub fn waste_text(&self) -> String {
        let (bought, parts, waste) = self.areas();
        format!(
            "stock {:.2} m², parts {:.2} m², waste {:.2} m² ({:.0}%)",
            bought / 1e6,
            parts / 1e6,
            waste / 1e6,
            100.0 * waste / bought.max(EPS)
        )
    }

    /// The layout as text: the pieces, then each sheet's pieces and its cuts
    /// in the order to saw them.
    pub fn report(&self) -> String {
        let mut out = String::new();
        out += &format!(
            "{} pieces {} thick on {}, gap {}{}{}\n{}\n\n",
            self.pieces.len(),
            number(self.pieces.first().map_or(0.0, |p| p.thickness)),
            self.bought(),
            number(self.gap),
            if self.trim > 0.0 {
                format!(
                    ", {} trimmed {}",
                    if self.edged { "ends" } else { "edges" },
                    number(self.trim)
                )
            } else {
                String::new()
            },
            if self.edged {
                ", long edges finished"
            } else {
                ""
            },
            self.waste_text()
        );
        out += " no  part                      size\n";
        for (i, p) in self.pieces.iter().enumerate() {
            out += &format!(
                "{:3}  {:24}  {} x {}{}\n",
                i + 1,
                p.name,
                number(p.length),
                number(p.width),
                if p.edge { "  edge shows" } else { "" }
            );
        }
        for (s, sheet) in self.sheets.iter().enumerate() {
            out += &format!(
                "\nsheet {} of {}: {} x {}, {:.0}% used, waste {:.2} m²\n",
                s + 1,
                self.sheets.len(),
                number(sheet.length),
                number(sheet.width),
                100.0 * sheet.used() / sheet.area(),
                (sheet.area() - sheet.used()) / 1e6
            );
            for p in &sheet.placed {
                out += &format!(
                    "  piece {:3} at x {} y {}, {} along x {} across{}\n",
                    p.piece + 1,
                    number(p.x),
                    number(p.y),
                    number(p.dx),
                    number(p.dy),
                    if p.on_edge {
                        "  (showing edge on the factory edge)"
                    } else {
                        ""
                    }
                );
            }
            for (c, cut) in sheet.cuts.iter().enumerate() {
                out += &format!("  cut {:3}  {}\n", c + 1, cut_text(cut));
            }
            for r in &sheet.free {
                if r.w >= 50.0 && r.h >= 50.0 {
                    out += &format!(
                        "  offcut {} x {} at x {} y {}\n",
                        number(r.w),
                        number(r.h),
                        number(r.x),
                        number(r.y)
                    );
                }
            }
        }
        out
    }
}

/// `rip at y 380, x 0-2400` or `cross at x 744, y 0-380`.
pub fn cut_text(cut: &Cut) -> String {
    let (along, across) = if cut.rip { ("y", "x") } else { ("x", "y") };
    format!(
        "{} at {along} {}, {across} {}-{}",
        if cut.rip { "rip" } else { "cross" },
        number(cut.at),
        number(cut.from),
        number(cut.to)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(name: &str, l: f64, w: f64) -> Piece {
        Piece {
            name: name.into(),
            length: l,
            width: w,
            thickness: 18.0,
            edge: false,
        }
    }

    #[test]
    fn pieces_never_overlap_and_keep_the_gap() {
        let pieces: Vec<Piece> = (0..9)
            .map(|i| {
                piece(
                    &format!("p{i}"),
                    300.0 + 50.0 * i as f64,
                    120.0 + 20.0 * i as f64,
                )
            })
            .collect();
        let o = Options {
            stocks: vec![(1200.0, 600.0)],
            gap: 4.0,
            trim: 5.0,
            grain: false,
            edged: false,
        };
        let layout = nest(pieces, o).unwrap();
        let placed: usize = layout.sheets.iter().map(|s| s.placed.len()).sum();
        assert_eq!(placed, 9);
        for s in &layout.sheets {
            for (i, a) in s.placed.iter().enumerate() {
                assert!(a.x >= 5.0 - EPS && a.y >= 5.0 - EPS);
                assert!(a.x + a.dx <= 1195.0 + EPS && a.y + a.dy <= 595.0 + EPS);
                for b in &s.placed[i + 1..] {
                    let apart = a.x + a.dx + 4.0 <= b.x + EPS
                        || b.x + b.dx + 4.0 <= a.x + EPS
                        || a.y + a.dy + 4.0 <= b.y + EPS
                        || b.y + b.dy + 4.0 <= a.y + EPS;
                    assert!(apart, "{a:?} and {b:?} are closer than the gap");
                }
            }
        }
    }

    #[test]
    fn grain_keeps_pieces_along_the_length() {
        let o = Options {
            stocks: vec![(2400.0, 600.0)],
            gap: 4.0,
            trim: 0.0,
            grain: true,
            edged: false,
        };
        let layout = nest(vec![piece("a", 500.0, 400.0)], o.clone()).unwrap();
        let p = &layout.sheets[0].placed[0];
        assert_eq!((p.dx, p.dy), (500.0, 400.0));
        assert!(
            nest(
                vec![piece("b", 590.0, 590.0)],
                Options {
                    stocks: vec![(2400.0, 500.0)],
                    ..o
                }
            )
            .is_err()
        );
    }

    #[test]
    fn edge_pieces_sit_on_a_finished_edge() {
        let mut pieces: Vec<Piece> = (0..4)
            .map(|i| piece(&format!("s{i}"), 744.0, 380.0))
            .collect();
        pieces.push(piece("rail", 744.0, 100.0));
        for p in pieces.iter_mut().take(4) {
            p.edge = true;
        }
        let o = Options {
            stocks: vec![(2440.0, 1220.0)],
            gap: 4.0,
            trim: 5.0,
            grain: true,
            edged: true,
        };
        let layout = nest(pieces, o.clone()).unwrap();
        for s in &layout.sheets {
            for p in &s.placed {
                if layout.pieces[p.piece].edge {
                    assert!(
                        p.y.abs() < EPS || (p.y + p.dy - s.width).abs() < EPS,
                        "{p:?} is off the edge"
                    );
                }
            }
        }
        assert!(nest(vec![piece("x", 10.0, 10.0)], Options { edged: false, ..o }).is_ok());
    }

    #[test]
    fn mixed_stock_buys_the_least_board() {
        let pieces = vec![piece("big", 2000.0, 1000.0), piece("small", 2000.0, 500.0)];
        let o = Options {
            stocks: vec![(2440.0, 1220.0), (2440.0, 600.0)],
            gap: 4.0,
            trim: 0.0,
            grain: true,
            edged: false,
        };
        let layout = nest(pieces, o).unwrap();
        let mut widths: Vec<f64> = layout.sheets.iter().map(|s| s.width).collect();
        widths.sort_by(f64::total_cmp);
        assert_eq!(widths, vec![600.0, 1220.0]);
        let (bought, parts, waste) = layout.areas();
        assert!((bought - parts - waste).abs() < 1e-6);
    }
}
