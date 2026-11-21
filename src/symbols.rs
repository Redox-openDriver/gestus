use alloc::string::String;
use alloc::vec::Vec;
use core::f32::consts::PI;

use crate::geometry::distance;

pub const POINTS: usize = 48;
const ONE_DIMENSIONAL: f32 = 0.3;
const SEARCH: f32 = 20.0 * PI / 180.0;
const PRECISION: f32 = 2.0 * PI / 180.0;
const HALF_DIAGONAL: f32 = 0.707_106_77;
const MAX_STROKE: usize = 4096;
const GOLDEN: f32 = 0.618_034;

#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub name: String,
    pub points: Vec<(f32, f32)>,
}

type Shape = [(f32, f32); POINTS];
type Stroke = Vec<(f32, f32)>;

fn resample(points: &[(f32, f32)]) -> Option<Shape> {
    if points.len() < 2 || points.len() > MAX_STROKE {
        return None;
    }
    let length: f32 = points.windows(2).map(|pair| distance(pair[1].0 - pair[0].0, pair[1].1 - pair[0].1)).sum();
    if !length.is_finite() || length <= f32::EPSILON {
        return None;
    }
    let interval = length / (POINTS - 1) as f32;
    let mut shape = [points[0]; POINTS];
    let (mut count, mut carried) = (1, 0.0f32);
    for pair in points.windows(2) {
        let (mut from, to) = (pair[0], pair[1]);
        let mut segment = distance(to.0 - from.0, to.1 - from.1);
        while carried + segment >= interval && count < POINTS {
            let t = (interval - carried) / segment;
            let point = (from.0 + t * (to.0 - from.0), from.1 + t * (to.1 - from.1));
            shape[count] = point;
            count += 1;
            segment -= interval - carried;
            from = point;
            carried = 0.0;
        }
        carried += segment;
    }
    let last = *points.last()?;
    for slot in &mut shape[count..] {
        *slot = last;
    }
    Some(shape)
}

fn fit(mut shape: Shape) -> Option<Shape> {
    let (mut left, mut right, mut top, mut bottom) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for (x, y) in shape {
        left = left.min(x);
        right = right.max(x);
        top = top.min(y);
        bottom = bottom.max(y);
    }
    let (width, height) = (right - left, bottom - top);
    let longest = width.max(height);
    if longest <= f32::EPSILON {
        return None;
    }
    let (sx, sy) = if width.min(height) / longest < ONE_DIMENSIONAL { (longest, longest) } else { (width, height) };
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    for point in &mut shape {
        *point = ((point.0 - left) / sx, (point.1 - top) / sy);
        cx += point.0;
        cy += point.1;
    }
    let (cx, cy) = (cx / POINTS as f32, cy / POINTS as f32);
    for point in &mut shape {
        *point = (point.0 - cx, point.1 - cy);
    }
    Some(shape)
}

fn shape_of(stroke: &[(f32, f32)]) -> Option<Shape> {
    fit(resample(stroke)?)
}

pub fn normalize(stroke: &[(f32, f32)]) -> Option<Vec<(f32, f32)>> {
    shape_of(stroke).map(|shape| shape.to_vec())
}

fn turned(shape: &Shape, angle: f32) -> Shape {
    let (sin, cos) = (libm::sinf(angle), libm::cosf(angle));
    let mut rotated = *shape;
    for point in &mut rotated {
        *point = (point.0 * cos - point.1 * sin, point.0 * sin + point.1 * cos);
    }
    rotated
}

fn apart(candidate: &Shape, template: &Shape, angle: f32) -> f32 {
    let Some(fitted) = fit(turned(candidate, angle)) else { return f32::MAX };
    let total: f32 = fitted.iter().zip(template).map(|(point, other)| distance(point.0 - other.0, point.1 - other.1)).sum();
    total / POINTS as f32
}

fn closest(candidate: &Shape, template: &Shape) -> f32 {
    let (mut low, mut high) = (-SEARCH, SEARCH);
    let mut first = GOLDEN * low + (1.0 - GOLDEN) * high;
    let mut second = (1.0 - GOLDEN) * low + GOLDEN * high;
    let (mut near, mut far) = (apart(candidate, template, first), apart(candidate, template, second));
    while high - low > PRECISION {
        if near < far {
            high = second;
            second = first;
            far = near;
            first = GOLDEN * low + (1.0 - GOLDEN) * high;
            near = apart(candidate, template, first);
        } else {
            low = first;
            first = second;
            near = far;
            second = (1.0 - GOLDEN) * low + GOLDEN * high;
            far = apart(candidate, template, second);
        }
    }
    near.min(far).min(apart(candidate, template, 0.0))
}

fn arc(cx: f32, cy: f32, radius: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    const STEPS: usize = 48;
    (0..=STEPS)
        .map(|i| {
            let angle = (from + (to - from) * i as f32 / STEPS as f32) * PI / 180.0;
            (cx + radius * libm::cosf(angle), cy + radius * libm::sinf(angle))
        })
        .collect()
}

fn curve(steps: usize, from: f32, to: f32, point: impl Fn(f32) -> (f32, f32)) -> Vec<(f32, f32)> {
    (0..=steps).map(|i| point(from + (to - from) * i as f32 / steps as f32)).collect()
}

fn both_ways(stroke: Stroke) -> [Stroke; 2] {
    let mut reversed = stroke.clone();
    reversed.reverse();
    [stroke, reversed]
}

fn built_in() -> Vec<(&'static str, Vec<Stroke>)> {
    let mut circles = Vec::new();
    for start in (0..8).map(|i| i as f32 * 45.0) {
        circles.push(arc(0.5, 0.5, 0.5, start, start + 360.0));
        circles.push(arc(0.5, 0.5, 0.5, start, start - 360.0));
    }
    let mut s = arc(0.5, 0.25, 0.25, -30.0, -270.0);
    s.extend(arc(0.5, 0.75, 0.25, -90.0, 150.0));
    let heart = curve(64, PI, 3.0 * PI, |t| {
        let sin = libm::sinf(t);
        (16.0 * sin * sin * sin, -(13.0 * libm::cosf(t) - 5.0 * libm::cosf(2.0 * t) - 2.0 * libm::cosf(3.0 * t) - libm::cosf(4.0 * t)))
    });
    let infinity = curve(64, 0.0, 2.0 * PI, |t| {
        let (sin, cos) = (libm::sinf(t), libm::cosf(t));
        (cos / (1.0 + sin * sin), sin * cos / (1.0 + sin * sin))
    });
    let star = [(0.19, 0.95), (0.5, 0.0), (0.81, 0.95), (0.0, 0.35), (1.0, 0.35), (0.19, 0.95)];
    alloc::vec![
        ("circle", circles),
        ("check", alloc::vec![alloc::vec![(0.0, 0.55), (0.35, 1.0), (1.0, 0.0)]]),
        ("caret", both_ways(alloc::vec![(0.0, 1.0), (0.5, 0.0), (1.0, 1.0)]).to_vec()),
        ("v", both_ways(alloc::vec![(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)]).to_vec()),
        ("less", both_ways(alloc::vec![(1.0, 0.0), (0.0, 0.5), (1.0, 1.0)]).to_vec()),
        ("greater", both_ways(alloc::vec![(0.0, 0.0), (1.0, 0.5), (0.0, 1.0)]).to_vec()),
        ("z", alloc::vec![alloc::vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]]),
        ("s", alloc::vec![s]),
        ("c", alloc::vec![arc(0.5, 0.5, 0.5, -40.0, -320.0)]),
        ("l", alloc::vec![alloc::vec![(0.0, 0.0), (0.0, 1.0), (0.6, 1.0)]]),
        ("m", alloc::vec![alloc::vec![(0.0, 1.0), (0.15, 0.0), (0.5, 0.8), (0.85, 0.0), (1.0, 1.0)]]),
        ("w", alloc::vec![alloc::vec![(0.0, 0.0), (0.25, 1.0), (0.5, 0.35), (0.75, 1.0), (1.0, 0.0)]]),
        ("triangle", both_ways(alloc::vec![(0.5, 0.0), (1.0, 1.0), (0.0, 1.0), (0.5, 0.0)]).to_vec()),
        ("square", both_ways(alloc::vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)]).to_vec()),
        ("star", both_ways(star.to_vec()).to_vec()),
        ("heart", both_ways(heart).to_vec()),
        ("infinity", both_ways(infinity).to_vec()),
    ]
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Recognizer {
    names: Vec<String>,
    shapes: Vec<(u16, Shape)>,
}

impl Recognizer {
    pub fn new(learned: &[Template]) -> Self {
        let mut recognizer = Recognizer::default();
        for (name, strokes) in built_in() {
            for stroke in strokes {
                recognizer.add(name, &stroke);
            }
        }
        for template in learned {
            recognizer.add(&template.name, &template.points);
        }
        recognizer
    }

    fn add(&mut self, name: &str, stroke: &[(f32, f32)]) {
        let Some(shape) = shape_of(stroke) else { return };
        let index = match self.names.iter().position(|known| known == name) {
            Some(index) => index,
            None => {
                self.names.push(String::from(name));
                self.names.len() - 1
            }
        };
        if let Ok(index) = u16::try_from(index) {
            self.shapes.push((index, shape));
        }
    }

    pub fn recognize(&self, stroke: &[(f32, f32)]) -> Option<(u16, f32)> {
        let candidate = resample(stroke)?;
        fit(candidate)?;
        self.shapes.iter().map(|(index, shape)| (*index, 1.0 - closest(&candidate, shape) / HALF_DIAGONAL)).max_by(|first, second| first.1.total_cmp(&second.1))
    }

    pub fn name(&self, index: u16) -> Option<&str> {
        self.names.get(usize::from(index)).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn sloppy(stroke: &[(f32, f32)], scale: (f32, f32), tilt: f32, wobble: f32) -> Vec<(f32, f32)> {
        let (sin, cos) = (libm::sinf(tilt * PI / 180.0), libm::cosf(tilt * PI / 180.0));
        let mut dense = Vec::new();
        for pair in stroke.windows(2) {
            for i in 0..12 {
                let t = i as f32 / 12.0;
                dense.push((pair[0].0 + (pair[1].0 - pair[0].0) * t, pair[0].1 + (pair[1].1 - pair[0].1) * t));
            }
        }
        dense.push(*stroke.last().unwrap());
        dense
            .iter()
            .enumerate()
            .map(|(i, (x, y))| {
                let (x, y) = (x * scale.0 + 20.0, y * scale.1 + 10.0);
                let shake = wobble * libm::sinf(i as f32 * 1.7);
                (x * cos - y * sin + shake, x * sin + y * cos - shake)
            })
            .collect()
    }

    #[test]
    fn every_built_in_symbol_drawn_sloppily_is_still_itself() {
        let recognizer = Recognizer::new(&[]);
        for (name, strokes) in built_in() {
            for (variant, stroke) in strokes.iter().enumerate() {
                let drawn = sloppy(stroke, (30.0, 24.0), 8.0, 0.3);
                let (index, score) = recognizer.recognize(&drawn).unwrap();
                assert_eq!(recognizer.name(index), Some(name), "{name} #{variant} scored {score}");
                assert!(score > 0.85, "{name} #{variant} scored {score}");
            }
        }
    }

    #[test]
    fn orientation_matters_so_less_and_caret_differ() {
        let recognizer = Recognizer::new(&[]);
        let less = sloppy(&[(1.0, 0.0), (0.0, 0.5), (1.0, 1.0)], (20.0, 25.0), 0.0, 0.0);
        let caret = sloppy(&[(0.0, 1.0), (0.5, 0.0), (1.0, 1.0)], (25.0, 20.0), 0.0, 0.0);
        let name = |stroke: &[(f32, f32)]| recognizer.recognize(stroke).and_then(|(index, _)| recognizer.name(index).map(String::from));
        assert_eq!(name(&less).as_deref(), Some("less"));
        assert_eq!(name(&caret).as_deref(), Some("caret"));
    }

    #[test]
    fn learned_symbols_join_and_a_dot_is_nothing() {
        let zigzag = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0), (4.0, 0.0), (5.0, 1.0)];
        let learned = Template { name: String::from("saw"), points: normalize(&zigzag).unwrap() };
        let recognizer = Recognizer::new(&[learned]);
        let (index, score) = recognizer.recognize(&sloppy(&zigzag, (6.0, 5.0), 3.0, 0.05)).unwrap();
        assert_eq!(recognizer.name(index), Some("saw"));
        assert!(score > 0.85, "{score}");
        assert_eq!(recognizer.recognize(&[(3.0, 3.0), (3.0, 3.0)]), None);
        assert_eq!(normalize(&[(1.0, 1.0)]), None);
    }
}
