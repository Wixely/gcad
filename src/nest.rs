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
}

/// One saw cut on a stock sheet. A rip runs along the stock's length at
/// `y = at`, from `x = from` to `to`; a crosscut runs across it at `x = at`,
/// from `y = from` to `to`. The kerf is taken from the far side of `at`.
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
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub length: f64,
    pub width: f64,
    pub gap: f64,
    pub trim: f64,
    pub pieces: Vec<Piece>,
    pub sheets: Vec<Stock>,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub length: f64,
    pub width: f64,
    /// The least space between pieces: the saw kerf, plus any allowance.
    pub gap: f64,
    /// Trimmed off every edge of the stock before cutting (chipped factory edges).
    pub trim: f64,
    /// Keep each piece's length along the stock's length (grain or pattern).
    pub grain: bool,
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

/// Packs the pieces onto as few sheets as it can, trying several orders and
/// ways to split the board, and keeps the layout with the fewest sheets, then
/// the biggest single offcut, then the fewest cuts.
pub fn nest(pieces: Vec<Piece>, options: Options) -> Result<Layout> {
    let Options {
        length,
        width,
        gap,
        trim,
        grain,
    } = options;
    if gap < 0.0 || trim < 0.0 {
        bail!("`gap` and `trim` cannot be negative");
    }
    let (ul, uw) = (length - 2.0 * trim, width - 2.0 * trim);
    for p in &pieces {
        let fits = (p.length <= ul + EPS && p.width <= uw + EPS)
            || (!grain && p.width <= ul + EPS && p.length <= uw + EPS);
        if !fits {
            bail!(
                "{} ({} x {}) is bigger than the {} x {} stock{}",
                p.name,
                number(p.length),
                number(p.width),
                number(length),
                number(width),
                if trim > 0.0 { " after trimming" } else { "" }
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
    let mut best: Option<(Vec<Stock>, (usize, f64, usize))> = None;
    for key in keys {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            let (ka, kb) = (key(&pieces[a]), key(&pieces[b]));
            kb.0.total_cmp(&ka.0)
                .then(kb.1.total_cmp(&ka.1))
                .then(a.cmp(&b))
        });
        for split in [
            Split::Shorter,
            Split::Longer,
            Split::RipFirst,
            Split::CrossFirst,
        ] {
            for fit in [Fit::ShortSide, Fit::Area] {
                let sheets = pack(&pieces, &order, options, split, fit);
                let biggest = sheets
                    .iter()
                    .flat_map(|s| &s.free)
                    .map(|r| r.w * r.h)
                    .fold(0.0, f64::max);
                let cuts: usize = sheets.iter().map(|s| s.cuts.len()).sum();
                let score = (sheets.len(), -biggest, cuts);
                let better = best.as_ref().is_none_or(|(_, b)| {
                    score
                        .0
                        .cmp(&b.0)
                        .then(score.1.total_cmp(&b.1))
                        .then(score.2.cmp(&b.2))
                        .is_lt()
                });
                if better {
                    best = Some((sheets, score));
                }
            }
        }
    }
    let sheets = best.map(|(s, _)| s).unwrap_or_default();
    Ok(Layout {
        length,
        width,
        gap,
        trim,
        pieces,
        sheets,
    })
}

fn pack(pieces: &[Piece], order: &[usize], o: Options, split: Split, fit: Fit) -> Vec<Stock> {
    let mut sheets: Vec<Stock> = Vec::new();
    for &i in order {
        let p = &pieces[i];
        let mut turns = vec![(p.length, p.width)];
        if !o.grain && (p.length - p.width).abs() > EPS {
            turns.push((p.width, p.length));
        }
        let mut choice: Option<(f64, f64, usize, usize, f64, f64)> = None;
        for (s, sheet) in sheets.iter().enumerate() {
            for (r, rect) in sheet.free.iter().enumerate() {
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
                        choice = Some((score.0, score.1, s, r, dx, dy));
                    }
                }
            }
        }
        let (s, r, dx, dy) = match choice {
            Some((_, _, s, r, dx, dy)) => (s, r, dx, dy),
            None => {
                sheets.push(Stock {
                    free: vec![Rect {
                        x: o.trim,
                        y: o.trim,
                        w: o.length - 2.0 * o.trim,
                        h: o.width - 2.0 * o.trim,
                    }],
                    ..Stock::default()
                });
                let rect = sheets.last().expect("just pushed").free[0];
                let &(dx, dy) = turns
                    .iter()
                    .find(|(dx, dy)| *dx <= rect.w + EPS && *dy <= rect.h + EPS)
                    .expect("checked that every piece fits the stock");
                (sheets.len() - 1, 0, dx, dy)
            }
        };
        let sheet = &mut sheets[s];
        let rect = sheet.free.remove(r);
        place(sheet, rect, i, dx, dy, o.gap, split);
    }
    sheets
}

fn place(sheet: &mut Stock, rect: Rect, piece: usize, dx: f64, dy: f64, gap: f64, split: Split) {
    sheet.placed.push(Placed {
        piece,
        x: rect.x,
        y: rect.y,
        dx,
        dy,
    });
    let (lw, lh) = (rect.w - dx, rect.h - dy);
    let rip_first = match split {
        Split::Shorter => lw > lh,
        Split::Longer => lw <= lh,
        Split::RipFirst => true,
        Split::CrossFirst => false,
    };
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
                at: rect.y + dy,
                from: rect.x,
                to: rect.x + rect.w,
            });
            keep(Rect {
                x: rect.x,
                y: rect.y + dy + gap,
                w: rect.w,
                h: lh - gap,
            });
        }
        if lw > EPS {
            cuts.push(Cut {
                rip: false,
                at: rect.x + dx,
                from: rect.y,
                to: rect.y + dy,
            });
            keep(Rect {
                x: rect.x + dx + gap,
                y: rect.y,
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
                at: rect.y + dy,
                from: rect.x,
                to: rect.x + dx,
            });
            keep(Rect {
                x: rect.x,
                y: rect.y + dy + gap,
                w: dx,
                h: lh - gap,
            });
        }
    }
    sheet.cuts.extend(cuts);
}

impl Layout {
    /// The layout as text: the pieces, then each sheet's pieces and its cuts
    /// in the order to saw them.
    pub fn report(&self) -> String {
        let mut out = String::new();
        let area: f64 = self.pieces.iter().map(|p| p.length * p.width).sum();
        let stock = self.length * self.width * self.sheets.len() as f64;
        out += &format!(
            "{} pieces {} thick on {} sheets of {} x {}, gap {}{}, {:.0}% used\n\n",
            self.pieces.len(),
            number(self.pieces.first().map_or(0.0, |p| p.thickness)),
            self.sheets.len(),
            number(self.length),
            number(self.width),
            number(self.gap),
            if self.trim > 0.0 {
                format!(", edges trimmed {}", number(self.trim))
            } else {
                String::new()
            },
            100.0 * area / stock.max(EPS)
        );
        out += " no  part                      size\n";
        for (i, p) in self.pieces.iter().enumerate() {
            out += &format!(
                "{:3}  {:24}  {} x {}\n",
                i + 1,
                p.name,
                number(p.length),
                number(p.width)
            );
        }
        for (s, sheet) in self.sheets.iter().enumerate() {
            out += &format!(
                "\nsheet {} of {}: {:.0}% used\n",
                s + 1,
                self.sheets.len(),
                100.0 * sheet.used() / (self.length * self.width)
            );
            for p in &sheet.placed {
                out += &format!(
                    "  piece {:3} at x {} y {}, {} along x {} across\n",
                    p.piece + 1,
                    number(p.x),
                    number(p.y),
                    number(p.dx),
                    number(p.dy)
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
            length: 1200.0,
            width: 600.0,
            gap: 4.0,
            trim: 5.0,
            grain: false,
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
            length: 2400.0,
            width: 600.0,
            gap: 4.0,
            trim: 0.0,
            grain: true,
        };
        let layout = nest(vec![piece("a", 500.0, 400.0)], o).unwrap();
        let p = &layout.sheets[0].placed[0];
        assert_eq!((p.dx, p.dy), (500.0, 400.0));
        assert!(
            nest(
                vec![piece("b", 590.0, 590.0)],
                Options { width: 500.0, ..o }
            )
            .is_err()
        );
    }
}
