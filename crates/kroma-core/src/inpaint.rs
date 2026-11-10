//! Content-Aware Fill / Remove / Spot Healing: multi-scale PatchMatch inpainting
//! (Barnes et al. 2009 nearest-neighbour field + Wexler-style EM voting).

use crate::buffer::{Image, Mask};
use crate::color::clamp01;
use rayon::prelude::*;

const R: i32 = 3; // patch radius (7x7)

struct Level {
    w: i32,
    h: i32,
    rgb: Vec<[f32; 3]>,
    hole: Vec<bool>,
    /// integral image of hole counts for O(1) patch validity
    integral: Vec<u32>,
}

impl Level {
    fn new(w: i32, h: i32, rgb: Vec<[f32; 3]>, hole: Vec<bool>) -> Level {
        let mut integral = vec![0u32; ((w + 1) * (h + 1)) as usize];
        for y in 0..h {
            let mut row = 0u32;
            for x in 0..w {
                row += hole[(y * w + x) as usize] as u32;
                integral[((y + 1) * (w + 1) + x + 1) as usize] =
                    integral[(y * (w + 1) + x + 1) as usize] + row;
            }
        }
        Level {
            w,
            h,
            rgb,
            hole,
            integral,
        }
    }
    #[inline]
    fn valid_src(&self, x: i32, y: i32) -> bool {
        if x - R < 0 || y - R < 0 || x + R >= self.w || y + R >= self.h {
            return false;
        }
        let w1 = self.w + 1;
        let (x0, y0, x1, y1) = (x - R, y - R, x + R + 1, y + R + 1);
        let s = self.integral[(y1 * w1 + x1) as usize] + self.integral[(y0 * w1 + x0) as usize]
            - self.integral[(y0 * w1 + x1) as usize]
            - self.integral[(y1 * w1 + x0) as usize];
        s == 0
    }
    #[inline]
    fn px(&self, x: i32, y: i32) -> [f32; 3] {
        let x = x.clamp(0, self.w - 1);
        let y = y.clamp(0, self.h - 1);
        self.rgb[(y * self.w + x) as usize]
    }
    fn dist(&self, tx: i32, ty: i32, sx: i32, sy: i32, cap: f32) -> f32 {
        let mut d = 0.0;
        for j in -R..=R {
            for i in -R..=R {
                let a = self.px(tx + i, ty + j);
                let b = self.px(sx + i, sy + j);
                d += (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
            }
            if d > cap {
                return d;
            }
        }
        d
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
}

fn downsample(l: &Level) -> Level {
    let (w, h) = ((l.w + 1) / 2, (l.h + 1) / 2);
    let mut rgb = vec![[0.0f32; 3]; (w * h) as usize];
    let mut hole = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            let mut n = 0.0;
            let mut any_hole = false;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = (x * 2 + dx, y * 2 + dy);
                if sx < l.w && sy < l.h {
                    let i = (sy * l.w + sx) as usize;
                    if l.hole[i] {
                        any_hole = true;
                    } else {
                        for c in 0..3 {
                            acc[c] += l.rgb[i][c];
                        }
                        n += 1.0;
                    }
                }
            }
            let i = (y * w + x) as usize;
            hole[i] = any_hole;
            if n > 0.0 {
                rgb[i] = [acc[0] / n, acc[1] / n, acc[2] / n];
            }
        }
    }
    Level::new(w, h, rgb, hole)
}

/// Fill holes by repeated neighbour averaging (initialisation at the coarsest scale).
fn diffuse(l: &mut Level) {
    let mut known: Vec<bool> = l.hole.iter().map(|h| !h).collect();
    let (w, h) = (l.w, l.h);
    loop {
        let mut changed = false;
        let mut next = known.clone();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if known[i] {
                    continue;
                }
                let mut acc = [0.0f32; 3];
                let mut n = 0.0;
                for (dx, dy) in [
                    (-1, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                    (-1, -1),
                    (1, 1),
                    (-1, 1),
                    (1, -1),
                ] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < w && ny < h && known[(ny * w + nx) as usize] {
                        let p = l.rgb[(ny * w + nx) as usize];
                        for c in 0..3 {
                            acc[c] += p[c];
                        }
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    l.rgb[i] = [acc[0] / n, acc[1] / n, acc[2] / n];
                    next[i] = true;
                    changed = true;
                }
            }
        }
        known = next;
        if !changed {
            break;
        }
    }
}

/// Inpaint pixels where `hole` >= 0.5. Returns a new image (alpha of hole pixels set to 1
/// where the surroundings are opaque).
pub fn content_aware_fill(img: &Image, hole: &Mask) -> Image {
    let w = img.width as i32;
    let holes: Vec<bool> = hole.data().iter().map(|v| *v >= 0.5).collect();
    if !holes.iter().any(|b| *b) {
        return img.clone();
    }
    // Work on a crop around the hole for speed (context = 3x hole size, min 64px margin).
    let hb = hole.bounds_above(0.5);
    let margin = (hb.w.max(hb.h) * 2).max(64);
    let crop = hb.inflate(margin).intersect(&img.bounds());
    let cw = crop.w;
    let ch = crop.h;
    let mut rgb = Vec::with_capacity((cw * ch) as usize);
    let mut hl = Vec::with_capacity((cw * ch) as usize);
    for y in 0..ch {
        for x in 0..cw {
            let p = img.get((crop.x + x) as u32, (crop.y + y) as u32);
            rgb.push([p[0], p[1], p[2]]);
            hl.push(holes[((crop.y + y) * w + crop.x + x) as usize]);
        }
    }
    let mut pyr = vec![Level::new(cw, ch, rgb, hl)];
    while pyr.last().unwrap().w.min(pyr.last().unwrap().h) > 48 && pyr.len() < 8 {
        let next = downsample(pyr.last().unwrap());
        pyr.push(next);
    }
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let nlev = pyr.len();
    diffuse(pyr.last_mut().unwrap());
    let mut nnf: Vec<(i32, i32)> = Vec::new();
    for li in (0..nlev).rev() {
        // initialise hole pixels from coarser level result
        if li + 1 < nlev {
            let (coarse_rgb, cw2) = (pyr[li + 1].rgb.clone(), pyr[li + 1].w);
            let lvl = &mut pyr[li];
            for y in 0..lvl.h {
                for x in 0..lvl.w {
                    let i = (y * lvl.w + x) as usize;
                    if lvl.hole[i] {
                        lvl.rgb[i] = coarse_rgb[((y / 2) * cw2 + x / 2) as usize];
                    }
                }
            }
        }
        let lvl_w = pyr[li].w;
        let lvl_h = pyr[li].h;
        // targets: hole pixels dilated by R
        let mut target = vec![false; (lvl_w * lvl_h) as usize];
        for y in 0..lvl_h {
            for x in 0..lvl_w {
                if pyr[li].hole[(y * lvl_w + x) as usize] {
                    for j in -R..=R {
                        for i in -R..=R {
                            let (nx, ny) = (x + i, y + j);
                            if nx >= 0 && ny >= 0 && nx < lvl_w && ny < lvl_h {
                                target[(ny * lvl_w + nx) as usize] = true;
                            }
                        }
                    }
                }
            }
        }
        // collect valid source positions for random init
        let sources: Vec<(i32, i32)> = (0..lvl_h)
            .flat_map(|y| (0..lvl_w).map(move |x| (x, y)))
            .filter(|&(x, y)| pyr[li].valid_src(x, y))
            .collect();
        if sources.is_empty() {
            continue;
        }
        // NNF init: upsample previous or random
        let prev = std::mem::take(&mut nnf);
        let prev_w = if li + 1 < nlev { pyr[li + 1].w } else { 0 };
        nnf = vec![(0, 0); (lvl_w * lvl_h) as usize];
        for y in 0..lvl_h {
            for x in 0..lvl_w {
                let i = (y * lvl_w + x) as usize;
                if !target[i] {
                    continue;
                }
                let mut cand = None;
                if !prev.is_empty() {
                    let p = prev[((y / 2).min(pyr[li + 1].h - 1) * prev_w + (x / 2).min(prev_w - 1))
                        as usize];
                    let c = (p.0 * 2 + x % 2, p.1 * 2 + y % 2);
                    if pyr[li].valid_src(c.0, c.1) {
                        cand = Some(c);
                    }
                }
                nnf[i] = cand.unwrap_or_else(|| sources[(rng.next() as usize) % sources.len()]);
            }
        }
        let em_iters = if li == 0 { 2 } else { 4 };
        for _em in 0..em_iters {
            let lvl = &pyr[li];
            let mut dist: Vec<f32> = vec![f32::MAX; (lvl_w * lvl_h) as usize];
            for y in 0..lvl_h {
                for x in 0..lvl_w {
                    let i = (y * lvl_w + x) as usize;
                    if target[i] {
                        dist[i] = lvl.dist(x, y, nnf[i].0, nnf[i].1, f32::MAX);
                    }
                }
            }
            for pass in 0..4 {
                let rev = pass % 2 == 1;
                let ys: Vec<i32> = if rev {
                    (0..lvl_h).rev().collect()
                } else {
                    (0..lvl_h).collect()
                };
                let d = if rev { -1 } else { 1 };
                for &y in &ys {
                    let xs: Box<dyn Iterator<Item = i32>> = if rev {
                        Box::new((0..lvl_w).rev())
                    } else {
                        Box::new(0..lvl_w)
                    };
                    for x in xs {
                        let i = (y * lvl_w + x) as usize;
                        if !target[i] {
                            continue;
                        }
                        let mut best = nnf[i];
                        let mut bd = dist[i];
                        // propagation
                        for (nx, ny) in [(x - d, y), (x, y - d)] {
                            if nx >= 0 && ny >= 0 && nx < lvl_w && ny < lvl_h {
                                let ni = (ny * lvl_w + nx) as usize;
                                if target[ni] {
                                    let c = (nnf[ni].0 + (x - nx), nnf[ni].1 + (y - ny));
                                    if lvl.valid_src(c.0, c.1) {
                                        let cd = lvl.dist(x, y, c.0, c.1, bd);
                                        if cd < bd {
                                            bd = cd;
                                            best = c;
                                        }
                                    }
                                }
                            }
                        }
                        // random search
                        let mut rad = lvl_w.max(lvl_h);
                        while rad >= 1 {
                            let c = (
                                rng.range(best.0 - rad, best.0 + rad),
                                rng.range(best.1 - rad, best.1 + rad),
                            );
                            if lvl.valid_src(c.0, c.1) {
                                let cd = lvl.dist(x, y, c.0, c.1, bd);
                                if cd < bd {
                                    bd = cd;
                                    best = c;
                                }
                            }
                            rad /= 2;
                        }
                        nnf[i] = best;
                        dist[i] = bd;
                    }
                }
            }
            // voting
            let mut sorted: Vec<f32> = dist.iter().cloned().filter(|d| *d < f32::MAX).collect();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let sigma2 = sorted
                .get(sorted.len() * 3 / 4)
                .cloned()
                .unwrap_or(1.0)
                .max(1e-4);
            let new_rgb: Vec<[f32; 3]> = (0..(lvl_w * lvl_h) as usize)
                .into_par_iter()
                .map(|i| {
                    if !lvl.hole[i] {
                        return lvl.rgb[i];
                    }
                    let (x, y) = (i as i32 % lvl_w, i as i32 / lvl_w);
                    let mut acc = [0.0f32; 3];
                    let mut wsum = 0.0;
                    for j in -R..=R {
                        for k in -R..=R {
                            let (tx, ty) = (x + k, y + j);
                            if tx < 0 || ty < 0 || tx >= lvl_w || ty >= lvl_h {
                                continue;
                            }
                            let ti = (ty * lvl_w + tx) as usize;
                            if !target[ti] {
                                continue;
                            }
                            let (sx, sy) = nnf[ti];
                            let c = lvl.px(sx - k, sy - j);
                            let wgt = (-dist[ti] / (2.0 * sigma2)).exp() + 1e-6;
                            for q in 0..3 {
                                acc[q] += c[q] * wgt;
                            }
                            wsum += wgt;
                        }
                    }
                    if wsum > 0.0 {
                        [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum]
                    } else {
                        lvl.rgb[i]
                    }
                })
                .collect();
            pyr[li].rgb = new_rgb;
        }
    }
    let fin = &pyr[0];
    let mut out = img.clone();
    let soft = hole.data();
    let op = out.pixels_mut();
    for y in 0..ch {
        for x in 0..cw {
            let gi = ((crop.y + y) * w + crop.x + x) as usize;
            let m = soft[gi];
            if m <= 0.0 {
                continue;
            }
            let c = fin.rgb[(y * cw + x) as usize];
            let p = &mut op[gi];
            for q in 0..3 {
                p[q] = clamp01(p[q] + (c[q] - p[q]) * m);
            }
            p[3] = p[3] + (1.0 - p[3]) * m;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fills_hole_with_surrounding_texture() {
        // stripes; punch a hole; the fill should be stripe-coloured, not mid-gray mush everywhere
        let mut img = Image::new(96, 96);
        for y in 0..96 {
            for x in 0..96 {
                let v = if (x / 6) % 2 == 0 { 0.1 } else { 0.9 };
                img.set(x, y, [v, v, v, 1.0]);
            }
        }
        let hole = crate::selection::rect_mask(96, 96, 40.0, 40.0, 56.0, 56.0);
        let mut damaged = img.clone();
        for y in 40..56 {
            for x in 40..56 {
                damaged.set(x, y, [1.0, 0.0, 0.0, 1.0]);
            }
        }
        let out = content_aware_fill(&damaged, &hole);
        let mut extreme = 0;
        for y in 42..54 {
            for x in 42..54 {
                let p = out.get(x, y);
                assert!(p[1] > 0.02 || p[0] < 0.5, "red left behind at {x},{y}");
                if p[0] < 0.3 || p[0] > 0.7 {
                    extreme += 1;
                }
            }
        }
        assert!(
            extreme > 100,
            "fill is texture-like ({extreme}/144 near stripe values)"
        );
    }
}
