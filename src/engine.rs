use alloc::vec;
use alloc::vec::Vec;

use crate::filter::OneEuro;
use crate::geometry::{degrees, direction, distance, seconds, turn};
use crate::inertia::{Glide, Kinetic, LIFT_GRACE, Recent, Velocity};
use crate::keys::{Action, Combo, Key, Slider, Toggle, steps};
use crate::settings::Config;
use crate::symbols::{Recognizer, Template};
use crate::taps::{Patterns, Tap};
use crate::zones::{Corner, Edge};
use crate::{Button, Event, Input, Notice, Phase, Surface, Touch};

pub(crate) const PERIOD: u64 = 8_000;
const DECIDE: f32 = 1.5;
const STROKE_STEP: f32 = 0.3;
const STROKE_LIMIT: usize = 2048;
const COVER_DRIFT: f32 = 20.0;
const COVER_LANDING: u64 = 150_000;
const COVER_QUIET: u64 = 1_000_000;
const COVER_MARGIN: f32 = 8.0;
const ROTATE_SCROLL: f32 = 0.25;
const TWO_HANDS: u64 = 400_000;
const REVERSALS: usize = 8;
const SETTLED: u64 = 50_000;
const TWIST_DECIDE: f32 = 5.0;
const TWIST_GIVE_UP: f32 = 10.0;
const PIVOT_MOVE: f32 = 0.5;
const ROTATE_START: f32 = 10.0;
const PINCH_START: f32 = 3.0;
const GATHER: u64 = 250_000;
const FLUNG: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Finger {
    id: u8,
    x: f32,
    y: f32,
    x0: f32,
    y0: f32,
    landed: u64,
    travelled: f32,
    palm: bool,
    edge: bool,
}

impl Finger {
    fn displacement(&self) -> f32 {
        distance(self.x - self.x0, self.y - self.y0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Idle,
    Pointer,
    Scroll,
    Swipe { fingers: u8, total: (f32, f32) },
    Switcher { travel: f32 },
    Drag,
    Rotate { last: f32, turned: f32 },
    Pinch { last: f32, spread: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Sequence {
    started: u64,
    most: u8,
    travelled: f32,
    clicked: bool,
    origin: (f32, f32),
    claimed: bool,
    hidden: bool,
    palmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    Button(Button),
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
enum Claim {
    None,
    Consumed,
    Deciding { id: u8 },
    Rail { ids: Vec<u8>, axis: Axis, slider: Slider, travel: f32 },
    Circle { id: u8, last: f32, turned: f32 },
    Joystick { id: u8, center: (f32, f32) },
    Symbol { id: u8, points: Vec<(f32, f32)> },
    Holding { id: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TapDrag {
    Idle,
    Waiting { until: u64 },
    Dragging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Carry {
    grace: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fling {
    glide: Glide,
    velocity: Velocity,
    ended: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Anchor {
    x: f32,
    y: f32,
    since: u64,
    travel: f32,
    fired: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Shaking {
    leg: f32,
    across: f32,
    sign: i8,
    reversals: [u64; REVERSALS],
    count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Covering {
    Holding { since: u64, id: u8, at: (f32, f32) },
    Covered,
    Spoiled,
}

pub struct Gestures {
    config: Config,
    surface: Surface,
    native: bool,
    patterns: Patterns,
    recognizer: Recognizer,
    window: u64,
    fingers: Vec<Finger>,
    working: Vec<u8>,
    mode: Mode,
    mode_began: u64,
    mode_travel: f32,
    pending: (f32, f32),
    pending_since: Option<u64>,
    split: [(f32, f32); 2],
    held: Option<Held>,
    held_ours: bool,
    sequence: Option<Sequence>,
    last: Option<u64>,
    claim: Claim,
    locked: bool,
    numpad: bool,
    tablet: bool,
    pen: bool,
    drawing: Option<u64>,
    queue: Vec<Tap>,
    queue_until: u64,
    tap_drag: TapDrag,
    carry: Option<Carry>,
    recent: Recent,
    fling: Option<Fling>,
    kinetic: Option<Kinetic>,
    pushed: Option<u64>,
    anchor: Option<Anchor>,
    shaking: Shaking,
    covering: Option<Covering>,
    touched: u64,
    filters: Vec<(u8, OneEuro, OneEuro)>,
    typed: Option<u64>,
}

fn by_fingers(count: usize) -> Button {
    match count {
        0 | 1 => Button::Left,
        2 => Button::Right,
        _ => Button::Middle,
    }
}

fn largest(list: &[(f32, f32)]) -> (f32, f32) {
    list.iter().copied().fold((0.0f32, 0.0f32), |best, step| if distance(step.0, step.1) > distance(best.0, best.1) { step } else { best })
}

fn press(combo: Combo, out: &mut Vec<Event>) {
    for key in combo.keys() {
        out.push(Event::Key { key, pressed: true });
    }
    for key in combo.keys().rev() {
        out.push(Event::Key { key, pressed: false });
    }
}

fn switch(forward: bool, out: &mut Vec<Event>) {
    if !forward {
        out.push(Event::Key { key: Key::Shift, pressed: true });
    }
    out.push(Event::Key { key: Key::Tab, pressed: true });
    out.push(Event::Key { key: Key::Tab, pressed: false });
    if !forward {
        out.push(Event::Key { key: Key::Shift, pressed: false });
    }
}

fn stroke_length(points: &[(f32, f32)]) -> f32 {
    points.windows(2).map(|pair| distance(pair[1].0 - pair[0].0, pair[1].1 - pair[0].1)).sum()
}

fn unlocking(locked: bool) -> impl Fn(&Action) -> bool {
    move |action| !locked || *action == Action::Toggle(Toggle::Touchpad)
}

impl Gestures {
    pub fn new(config: impl Into<Config>, surface: Surface) -> Self {
        Gestures::with_symbols(config.into(), surface, &[])
    }

    pub fn with_symbols(config: Config, surface: Surface, learned: &[Template]) -> Self {
        let patterns = Patterns::new(&config, surface, true);
        let recognizer = if config.symbols.enabled { Recognizer::new(learned) } else { Recognizer::default() };
        let window = if config.knock.enabled { config.taps.window.max(config.knock.window) } else { config.taps.window };
        Gestures {
            config,
            surface,
            native: false,
            patterns,
            recognizer,
            window,
            fingers: Vec::new(),
            working: Vec::new(),
            mode: Mode::Idle,
            mode_began: 0,
            mode_travel: 0.0,
            pending: (0.0, 0.0),
            pending_since: None,
            split: [(0.0, 0.0); 2],
            held: None,
            held_ours: false,
            sequence: None,
            last: None,
            claim: Claim::None,
            locked: false,
            numpad: false,
            tablet: false,
            pen: false,
            drawing: None,
            queue: Vec::new(),
            queue_until: 0,
            tap_drag: TapDrag::Idle,
            carry: None,
            recent: Recent::default(),
            fling: None,
            kinetic: None,
            pushed: None,
            anchor: None,
            shaking: Shaking::default(),
            covering: None,
            touched: 0,
            filters: Vec::new(),
            typed: None,
        }
    }

    pub fn native(mut self) -> Self {
        self.native = true;
        self.patterns = Patterns::new(&self.config, self.surface, false);
        self
    }

    pub fn hidden(&self) -> bool {
        self.locked || self.tablet || self.claim != Claim::None
    }

    pub fn visible(&self) -> Vec<Touch> {
        if self.hidden() {
            return Vec::new();
        }
        self.fingers.iter().map(|finger| Touch { id: finger.id, x: finger.x, y: finger.y, palm: finger.palm }).collect()
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn numpad(&self) -> bool {
        self.numpad
    }

    pub fn tablet(&self) -> bool {
        self.tablet
    }

    pub fn symbol(&self, index: u16) -> Option<&str> {
        self.recognizer.name(index)
    }

    pub fn typed(&mut self, time: u64) {
        self.typed = Some(time);
    }

    fn typing(&self, time: u64) -> bool {
        self.config.typing.enabled && !self.native && self.typed.is_some_and(|at| time.saturating_sub(at) < self.config.typing.quiet)
    }

    pub fn deadline(&self) -> Option<u64> {
        let mut soonest: Option<u64> = None;
        let mut consider = |at: Option<u64>| {
            if let Some(at) = at {
                soonest = Some(soonest.map_or(at, |known| known.min(at)));
            }
        };
        consider((!self.queue.is_empty() && self.sequence.is_none()).then_some(self.queue_until));
        consider(match self.tap_drag {
            TapDrag::Waiting { until } if self.sequence.is_none() => Some(until),
            _ => None,
        });
        consider(self.carry.and_then(|carry| carry.grace));
        consider(self.drawing);
        consider(self.hold_target().map(|(_, at)| at));
        consider(self.cover_at());
        consider(self.dwell_at());
        consider(self.kinetic.map(|kinetic| kinetic.at.saturating_add(PERIOD)));
        consider(self.pushed.map(|at| at.saturating_add(PERIOD)));
        soonest
    }

    pub fn tick(&mut self, now: u64) -> Vec<Event> {
        let mut out = Vec::new();
        if self.last.is_none_or(|last| now >= last) {
            self.due(now, &mut out);
        }
        out
    }

    pub fn feed(&mut self, input: &Input) -> Vec<Event> {
        let mut out = Vec::new();
        let time = input.time;
        self.due(time, &mut out);
        let elapsed = self.last.map_or(0, |last| time.saturating_sub(last));
        self.last = Some(time);
        let touches = self.smooth(&input.touches, time);
        let (moves, touched, joined) = self.track(&touches, time);
        if touched {
            self.kinetic = None;
            self.fling = None;
        }
        if self.tablet {
            self.tablet_frame(input.button, time, &mut out);
        } else {
            self.frame(input.button, time, elapsed, &moves, joined, &mut out);
        }
        out
    }

    fn due(&mut self, now: u64, out: &mut Vec<Event>) {
        if !self.queue.is_empty() && now >= self.queue_until && (self.sequence.is_none() || !self.queue_candidate(now)) {
            self.flush(now, out);
        }
        if let TapDrag::Waiting { until } = self.tap_drag
            && now >= until
            && self.sequence.is_none()
        {
            self.tap_drag = TapDrag::Idle;
            out.push(Event::Release(Button::Left));
        }
        if let Some(Carry { grace: Some(until) }) = self.carry
            && now >= until
        {
            self.carry = None;
            out.push(Event::Release(Button::Left));
        }
        if let Some(until) = self.drawing
            && now >= until
        {
            self.drawing = None;
            out.push(Event::Notice(Notice::Cancelled));
        }
        self.check_hold(now, out);
        self.check_cover(now, out);
        self.check_dwell(now, out);
        if let Some(mut kinetic) = self.kinetic
            && now >= kinetic.at.saturating_add(PERIOD)
        {
            let ((dx, dy), alive) = kinetic.step(now);
            out.push(match kinetic.glide {
                Glide::Scroll => Event::Scroll { dx, dy },
                Glide::Pointer => Event::Motion { dx, dy },
            });
            self.kinetic = alive.then_some(kinetic);
        }
        if self.pushed.is_some_and(|at| now >= at.saturating_add(PERIOD)) {
            self.push(now, out);
        }
    }

    fn smooth(&mut self, touches: &[Touch], time: u64) -> Vec<Touch> {
        let smoothing = self.config.smoothing;
        if !smoothing.enabled {
            return touches.to_vec();
        }
        self.filters.retain(|(id, _, _)| touches.iter().any(|touch| touch.id == *id));
        touches
            .iter()
            .map(|touch| {
                let index = match self.filters.iter().position(|(id, _, _)| *id == touch.id) {
                    Some(index) => index,
                    None => {
                        self.filters.push((touch.id, OneEuro::default(), OneEuro::default()));
                        self.filters.len() - 1
                    }
                };
                let (_, x, y) = &mut self.filters[index];
                Touch { x: x.filter(touch.x, time, smoothing.cutoff, smoothing.beta), y: y.filter(touch.y, time, smoothing.cutoff, smoothing.beta), ..*touch }
            })
            .collect()
    }

    fn track(&mut self, touches: &[Touch], time: u64) -> (Vec<(u8, f32, f32)>, bool, bool) {
        let mut next: Vec<Finger> = Vec::with_capacity(touches.len());
        let mut moves: Vec<(u8, f32, f32)> = Vec::with_capacity(touches.len());
        let (mut touched, mut joined, mut palmed) = (false, false, false);
        if self.fingers.is_empty() {
            self.touched = time;
        }
        for touch in touches {
            let finger = match self.fingers.iter().find(|finger| finger.id == touch.id) {
                Some(previous) => {
                    let (dx, dy) = (touch.x - previous.x, touch.y - previous.y);
                    moves.push((touch.id, dx, dy));
                    palmed |= touch.palm && !previous.palm;
                    let mut moved =
                        Finger { x: touch.x, y: touch.y, travelled: previous.travelled + distance(dx, dy), palm: previous.palm || touch.palm, ..*previous };
                    moved.edge &= moved.displacement() < self.config.pointer.edge_release;
                    moved
                }
                None => {
                    touched = true;
                    joined |= !touch.palm;
                    Finger {
                        id: touch.id,
                        x: touch.x,
                        y: touch.y,
                        x0: touch.x,
                        y0: touch.y,
                        landed: time,
                        travelled: 0.0,
                        palm: touch.palm,
                        edge: self.at_edge(touch.x, touch.y),
                    }
                }
            };
            next.push(finger);
        }
        self.fingers = next;
        if palmed && let Some(sequence) = self.sequence.as_mut() {
            sequence.palmed = true;
        }
        (moves, touched, joined)
    }

    fn frame(&mut self, button: bool, time: u64, elapsed: u64, moves: &[(u8, f32, f32)], joined: bool, out: &mut Vec<Event>) {
        let fingers: Vec<Finger> = self.fingers.iter().copied().filter(|finger| !finger.palm).collect();
        let candidates: Vec<Finger> = fingers.iter().copied().filter(|finger| !finger.edge).collect();
        let working: Vec<Finger> = candidates.iter().copied().filter(|finger| !self.thumb(finger, time, candidates.len() - 1)).collect();
        let ids: Vec<u8> = working.iter().map(|finger| finger.id).collect();
        let same_fingers = ids == self.working;
        self.working = ids;

        let starting = self.sequence.is_none() && !fingers.is_empty();
        if !fingers.is_empty() {
            let origin = (fingers[0].x, fingers[0].y);
            let sequence = self.sequence.get_or_insert(Sequence {
                started: time,
                most: 0,
                travelled: 0.0,
                clicked: false,
                origin,
                claimed: false,
                hidden: false,
                palmed: false,
            });
            sequence.most = sequence.most.max(working.len().max(1) as u8);
            sequence.travelled = working.iter().fold(sequence.travelled, |most, finger| most.max(finger.travelled));
            if self.fling.is_some_and(|fling| time.saturating_sub(fling.ended) > LIFT_GRACE) {
                self.fling = None;
            }
        }
        if starting {
            self.begin(&fingers, time, out);
        } else if joined {
            self.join(&fingers, time, out);
        }
        let (mut withheld, mut catch_up) = (false, false);
        if !self.queue.is_empty() && self.sequence.is_some() {
            if self.queue_candidate(time) {
                withheld = true;
            } else {
                self.flush(time, out);
                catch_up = true;
            }
        }
        if self.locked {
            self.check_hold(time, out);
            if fingers.is_empty() {
                self.lifted(time, out);
            }
            return;
        }

        self.buttons(button, &fingers, &working, time, out);
        if !same_fingers {
            self.end_mode(time, out);
        }
        if !fingers.is_empty() {
            let free = self.claimed(moves, elapsed, time, out);
            if self.native {
                if free
                    && same_fingers
                    && let [finger] = working.as_slice()
                {
                    let step = moves.iter().find(|(id, _, _)| *id == finger.id).map_or((0.0, 0.0), |(_, dx, dy)| (*dx, *dy));
                    self.shake(step, time, out);
                }
            } else if free && same_fingers && catch_up && working.len() == 1 {
                let finger = working[0];
                self.enter(Mode::Pointer, time);
                self.point((finger.x - finger.x0, finger.y - finger.y0), elapsed, time, 1.0, out);
            } else if free && same_fingers && !withheld {
                self.movement(&working, moves, elapsed, time, out);
            }
        }
        self.track_dwell(time);
        self.check_hold(time, out);
        self.check_cover(time, out);
        self.check_dwell(time, out);
        self.push(time, out);
        if fingers.is_empty() {
            self.lifted(time, out);
        }
    }

    fn mark_claimed(&mut self) {
        if let Some(sequence) = self.sequence.as_mut() {
            sequence.claimed = true;
        }
    }

    fn mark_hidden(&mut self) {
        if let Some(sequence) = self.sequence.as_mut() {
            sequence.hidden = true;
        }
    }

    fn mark_shown(&mut self) {
        if let Some(sequence) = self.sequence.as_mut() {
            sequence.hidden = false;
        }
    }

    fn enter(&mut self, mode: Mode, time: u64) {
        if core::mem::discriminant(&mode) != core::mem::discriminant(&self.mode) {
            self.mode_began = time;
            self.mode_travel = 0.0;
        }
        self.mode = mode;
    }

    fn begin(&mut self, fingers: &[Finger], time: u64, out: &mut Vec<Event>) {
        if self.covering == Some(Covering::Covered) {
            self.claim = Claim::Consumed;
            self.mark_claimed();
            return;
        }
        self.claim = Claim::None;
        self.anchor = None;
        self.shaking = Shaking::default();
        let first = fingers[0];
        let typing = self.typing(time);
        if !self.queue.is_empty() && (typing || fingers.len() > 1 || !self.patterns.continues(&self.queue, first.x, first.y, unlocking(self.locked))) {
            self.flush(time, out);
        }
        match self.tap_drag {
            TapDrag::Waiting { .. } if fingers.len() == 1 && !self.locked && !typing => self.tap_drag = TapDrag::Dragging,
            TapDrag::Waiting { .. } => {
                self.tap_drag = TapDrag::Idle;
                out.push(Event::Release(Button::Left));
            }
            _ => {}
        }
        if typing {
            self.claim = Claim::Consumed;
            self.mark_claimed();
            return;
        }
        if let Some(carry) = self.carry.as_mut() {
            carry.grace = None;
        }
        if self.locked {
            self.mark_hidden();
            return;
        }
        if fingers.len() > 1 {
            self.two_hands(fingers, time, out);
            return;
        }
        if self.drawing.is_some() && self.config.symbols.enabled {
            self.drawing = None;
            self.claim = Claim::Symbol { id: first.id, points: vec![(first.x, first.y)] };
            self.mark_claimed();
            self.mark_hidden();
            return;
        }
        let joystick = self.config.joystick;
        if joystick.enabled && joystick.corner.contains(first.x, first.y, self.surface, joystick.size) {
            self.claim = Claim::Joystick { id: first.id, center: (first.x, first.y) };
            self.mark_hidden();
            return;
        }
        if self.native && (self.numpad || self.patterns.starts(&[Tap { fingers: 1, x: first.x, y: first.y, at: time }], unlocking(self.locked))) {
            self.claim = Claim::Holding { id: first.id };
            self.mark_hidden();
            return;
        }
        if self.zoned(first.x, first.y) {
            self.claim = Claim::Deciding { id: first.id };
            self.mark_hidden();
        }
    }

    fn join(&mut self, fingers: &[Finger], time: u64, out: &mut Vec<Event>) {
        if self.locked {
            return;
        }
        match &self.claim {
            Claim::Symbol { points, .. } if stroke_length(points) < self.config.pointer.tap_travel => {
                self.claim = Claim::None;
                if let Some(sequence) = self.sequence.as_mut() {
                    sequence.claimed = false;
                }
                out.push(Event::Notice(Notice::Cancelled));
            }
            Claim::Symbol { .. } => {
                self.claim = Claim::Consumed;
                out.push(Event::Notice(Notice::Unrecognized));
                return;
            }
            Claim::Deciding { .. } | Claim::Joystick { .. } | Claim::Holding { .. } => {
                self.claim = Claim::None;
                self.mark_shown();
            }
            Claim::Rail { .. } | Claim::Circle { .. } => {
                self.claim = Claim::Consumed;
                return;
            }
            Claim::None | Claim::Consumed => {}
        }
        if fingers.len() == 2 {
            self.two_hands(fingers, time, out);
        }
    }

    fn rail_slider(&self, edge: Edge) -> Slider {
        let rails = &self.config.rails;
        match edge {
            Edge::Left => rails.left,
            Edge::Right => rails.right,
            Edge::Top => rails.top,
            Edge::Bottom => rails.bottom,
        }
    }

    fn edge_swipe(&self, edge: Edge) -> Option<Action> {
        let swipes = &self.config.edge_swipes;
        match edge {
            Edge::Left => swipes.left,
            Edge::Right => swipes.right,
            Edge::Top => swipes.top,
            Edge::Bottom => swipes.bottom,
        }
    }

    fn zoned(&self, x: f32, y: f32) -> bool {
        let config = &self.config;
        let rails =
            config.rails.enabled && Edge::ALL.iter().any(|edge| self.rail_slider(*edge) != Slider::Off && edge.near(x, y, self.surface, config.rails.width));
        let near = |edges: [Edge; 2]| edges.iter().any(|edge| edge.near(x, y, self.surface, config.rails.width));
        let hands = self.native
            && config.rails.enabled
            && ((config.rails.sides != Slider::Off && near([Edge::Left, Edge::Right]))
                || (config.rails.top_bottom != Slider::Off && near([Edge::Top, Edge::Bottom])));
        let circular = config.circular.enabled && Edge::ALL.iter().any(|edge| edge.near(x, y, self.surface, config.circular.width));
        let swipes = config.edge_swipes.enabled
            && Edge::ALL.iter().any(|edge| self.edge_swipe(*edge).is_some() && edge.near(x, y, self.surface, config.edge_swipes.width));
        rails || hands || circular || swipes
    }

    fn two_hands(&mut self, fingers: &[Finger], time: u64, out: &mut Vec<Event>) {
        let rails = self.config.rails;
        if !rails.enabled || fingers.len() != 2 {
            return;
        }
        let (first, second) = if fingers[0].landed <= fingers[1].landed { (fingers[0], fingers[1]) } else { (fingers[1], fingers[0]) };
        if first.displacement() > self.config.pointer.tap_travel || second.landed.saturating_sub(first.landed) > TWO_HANDS {
            return;
        }
        let near = |finger: &Finger, edge: Edge| edge.near(finger.x, finger.y, self.surface, rails.width);
        let opposite = |one: Edge, other: Edge| (near(&first, one) && near(&second, other)) || (near(&first, other) && near(&second, one));
        let chosen = if rails.sides != Slider::Off && opposite(Edge::Left, Edge::Right) {
            Some((Axis::Vertical, rails.sides))
        } else if rails.top_bottom != Slider::Off && opposite(Edge::Top, Edge::Bottom) {
            Some((Axis::Horizontal, rails.top_bottom))
        } else {
            None
        };
        if let Some((axis, slider)) = chosen {
            self.end_mode(time, out);
            self.claim = Claim::Rail { ids: vec![first.id, second.id], axis, slider, travel: 0.0 };
            self.mark_claimed();
        }
    }

    fn claimed(&mut self, moves: &[(u8, f32, f32)], elapsed: u64, time: u64, out: &mut Vec<Event>) -> bool {
        let claim = core::mem::replace(&mut self.claim, Claim::None);
        let (claim, free) = match claim {
            Claim::None => (Claim::None, true),
            Claim::Consumed => (Claim::Consumed, false),
            Claim::Deciding { id } => self.decide(id, elapsed, time, out),
            Claim::Rail { ids, axis, slider, travel } => {
                let travel = self.rail(&ids, axis, slider, travel, moves, time, out);
                (Claim::Rail { ids, axis, slider, travel }, false)
            }
            Claim::Circle { id, last, turned } => (self.circle(id, last, turned, time, out), false),
            Claim::Joystick { id, center } => (Claim::Joystick { id, center }, false),
            Claim::Symbol { id, mut points } => {
                self.stroke(id, &mut points);
                (Claim::Symbol { id, points }, false)
            }
            Claim::Holding { id } => {
                let tap_travel = self.config.pointer.tap_travel;
                let still = self.sequence.is_some_and(|sequence| sequence.most == 1 && !sequence.clicked && sequence.travelled <= tap_travel);
                (if still { Claim::Holding { id } } else { Claim::None }, false)
            }
        };
        if self.claim != Claim::Consumed {
            if claim == Claim::None {
                self.mark_shown();
            }
            self.claim = claim;
        }
        free
    }

    fn decide(&mut self, id: u8, elapsed: u64, time: u64, out: &mut Vec<Event>) -> (Claim, bool) {
        let Some(finger) = self.fingers.iter().copied().find(|finger| finger.id == id) else { return (Claim::None, true) };
        if self.native && self.held.is_some() {
            return (Claim::None, false);
        }
        let (dx, dy) = (finger.x - finger.x0, finger.y - finger.y0);
        let swipes = self.config.edge_swipes;
        let fresh = swipes.enabled && time.saturating_sub(finger.landed) <= swipes.time;
        let swipe_edges: Vec<(Edge, Action)> = Edge::ALL
            .iter()
            .filter_map(|edge| self.edge_swipe(*edge).map(|action| (*edge, action)))
            .filter(|(edge, _)| fresh && edge.near(finger.x0, finger.y0, self.surface, swipes.width))
            .collect();
        for (edge, action) in &swipe_edges {
            let (inward, across) = edge.inward(dx, dy);
            if inward >= swipes.distance && inward >= 2.0 * across {
                self.mark_claimed();
                self.act(*action, time, out);
                return (Claim::Consumed, false);
            }
        }
        if distance(dx, dy) < DECIDE {
            return (Claim::Deciding { id }, false);
        }
        let circular = self.config.circular;
        if circular.enabled
            && Edge::ALL.iter().any(|edge| edge.near(finger.x0, finger.y0, self.surface, circular.width))
            && self.tangential(finger.x0, finger.y0, dx, dy)
        {
            self.mark_claimed();
            self.end_mode(time, out);
            let start = self.around(finger.x0, finger.y0);
            return (self.circle(id, start, 0.0, time, out), false);
        }
        let rails = self.config.rails;
        if rails.enabled {
            for edge in Edge::ALL {
                let slider = self.rail_slider(edge);
                if slider == Slider::Off || !edge.near(finger.x0, finger.y0, self.surface, rails.width) {
                    continue;
                }
                let along = if edge.vertical() { dy.abs() >= 2.0 * dx.abs() } else { dx.abs() >= 2.0 * dy.abs() };
                if along {
                    self.mark_claimed();
                    self.end_mode(time, out);
                    let axis = if edge.vertical() { Axis::Vertical } else { Axis::Horizontal };
                    let travel = self.rail_step(axis, slider, 0.0, (dx, dy), time, out);
                    return (Claim::Rail { ids: vec![id], axis, slider, travel }, false);
                }
            }
        }
        if swipe_edges.iter().any(|(edge, _)| {
            let (inward, across) = edge.inward(dx, dy);
            inward > across
        }) {
            return (Claim::Deciding { id }, false);
        }
        if !self.native && !finger.edge && self.working.contains(&id) {
            self.enter(Mode::Pointer, time);
            self.point((dx, dy), elapsed, time, 1.0, out);
        }
        (Claim::None, false)
    }

    fn around(&self, x: f32, y: f32) -> f32 {
        direction((x - self.surface.width / 2.0) / self.surface.width, (y - self.surface.height / 2.0) / self.surface.height)
    }

    fn tangential(&self, x: f32, y: f32, dx: f32, dy: f32) -> bool {
        let radial = ((x - self.surface.width / 2.0) / self.surface.width, (y - self.surface.height / 2.0) / self.surface.height);
        let reach = distance(radial.0, radial.1);
        if reach <= f32::EPSILON {
            return false;
        }
        let unit = (radial.0 / reach, radial.1 / reach);
        let step = (dx / self.surface.width, dy / self.surface.height);
        let outward = step.0 * unit.0 + step.1 * unit.1;
        distance(step.0 - outward * unit.0, step.1 - outward * unit.1) >= outward.abs()
    }

    fn circle(&mut self, id: u8, last: f32, turned: f32, time: u64, out: &mut Vec<Event>) -> Claim {
        let Some(finger) = self.fingers.iter().copied().find(|finger| finger.id == id) else { return Claim::Consumed };
        let angle = self.around(finger.x, finger.y);
        let delta = degrees(turn(last, angle));
        let circular = self.config.circular;
        let turned = self.slide(circular.action, turned, delta, circular.step, (0.0, delta / 360.0 * circular.turn), time, out);
        Claim::Circle { id, last: angle, turned }
    }

    #[allow(clippy::too_many_arguments)]
    fn rail(&mut self, ids: &[u8], axis: Axis, slider: Slider, travel: f32, moves: &[(u8, f32, f32)], time: u64, out: &mut Vec<Event>) -> f32 {
        let deltas: Vec<(f32, f32)> = moves.iter().filter(|(id, _, _)| ids.contains(id)).map(|(_, dx, dy)| (*dx, *dy)).collect();
        if deltas.is_empty() {
            return travel;
        }
        let count = deltas.len() as f32;
        let mean = (deltas.iter().map(|step| step.0).sum::<f32>() / count, deltas.iter().map(|step| step.1).sum::<f32>() / count);
        self.rail_step(axis, slider, travel, mean, time, out)
    }

    fn rail_step(&mut self, axis: Axis, slider: Slider, travel: f32, (dx, dy): (f32, f32), time: u64, out: &mut Vec<Event>) -> f32 {
        let step = self.config.rails.step;
        match axis {
            Axis::Vertical => self.slide(slider, travel, -dy, step, (0.0, dy), time, out),
            Axis::Horizontal => self.slide(slider, travel, dx, step, (dx, 0.0), time, out),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn slide(&mut self, slider: Slider, accumulated: f32, delta: f32, step: f32, scroll: (f32, f32), time: u64, out: &mut Vec<Event>) -> f32 {
        match slider {
            Slider::Off => accumulated,
            Slider::Scroll => {
                self.scroll(scroll, 0, time, out);
                accumulated
            }
            Slider::Steps { forward, back } => {
                let (count, rest) = steps(accumulated, delta, step);
                let action = if count > 0 { forward } else { back };
                for _ in 0..count.unsigned_abs() {
                    self.act(action, time, out);
                }
                rest
            }
        }
    }

    fn stroke(&self, id: u8, points: &mut Vec<(f32, f32)>) {
        let Some(finger) = self.fingers.iter().find(|finger| finger.id == id) else { return };
        let last = points.last().copied().unwrap_or((finger.x, finger.y));
        if points.len() < STROKE_LIMIT && distance(finger.x - last.0, finger.y - last.1) >= STROKE_STEP {
            points.push((finger.x, finger.y));
        }
    }

    fn movement(&mut self, working: &[Finger], moves: &[(u8, f32, f32)], elapsed: u64, time: u64, out: &mut Vec<Event>) {
        let working_moves: Vec<(f32, f32)> = moves.iter().filter(|(id, _, _)| self.working.contains(id)).map(|(_, dx, dy)| (*dx, *dy)).collect();
        if working_moves.is_empty() {
            return;
        }
        let pressed = matches!(self.held, Some(Held::Button(_)));
        let count = working.len();
        let n = working_moves.len() as f32;
        let mean = (working_moves.iter().map(|step| step.0).sum::<f32>() / n, working_moves.iter().map(|step| step.1).sum::<f32>() / n);
        if pressed || count == 1 {
            self.enter(Mode::Pointer, time);
            self.point(largest(&working_moves), elapsed, time, 1.0, out);
        } else if count == 2 {
            self.two_fingers(working, moves, mean, elapsed, time, out);
        } else if count >= 3 {
            self.many_fingers(count, mean, elapsed, time, out);
        }
    }

    fn two_fingers(&mut self, working: &[Finger], moves: &[(u8, f32, f32)], mean: (f32, f32), elapsed: u64, time: u64, out: &mut Vec<Event>) {
        let pair: Vec<(f32, f32)> =
            working.iter().map(|finger| moves.iter().find(|(id, _, _)| *id == finger.id).map_or((0.0, 0.0), |(_, dx, dy)| (*dx, *dy))).collect();
        let sniper = self.config.sniper;
        let factor = if sniper.enabled && working.iter().any(|finger| finger.displacement() < self.config.pointer.rest_travel) { sniper.factor } else { 1.0 };
        match self.mode {
            Mode::Scroll => self.scroll(mean, elapsed, time, out),
            Mode::Pointer => self.point(largest(&pair), elapsed, time, factor, out),
            Mode::Rotate { .. } => self.rotate(working, time, out),
            Mode::Pinch { .. } => self.pinch(working, time, out),
            _ => {
                for (sum, step) in self.split.iter_mut().zip(&pair) {
                    *sum = (sum.0 + step.0, sum.1 + step.1);
                }
                let [a, b] = self.split;
                let (first, second) = (distance(a.0, a.1), distance(b.0, b.1));
                let most = first.max(second);
                let twisting = self.config.rotate.enabled || self.config.pinch.enabled;
                if most >= self.config.pointer.scroll_start {
                    let both = first.min(second) >= self.config.pointer.follow;
                    if both && a.0 * b.0 + a.1 * b.1 > 0.0 {
                        self.enter(Mode::Scroll, time);
                        self.scroll(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), elapsed, time, out);
                    } else if !twisting {
                        self.enter(Mode::Pointer, time);
                        let split = self.split;
                        self.point(largest(&split), elapsed, time, factor, out);
                    } else {
                        let resting = most >= TWIST_DECIDE && first.min(second) < PIVOT_MOVE;
                        match self.twist(working, a, b) {
                            Some(mode @ Mode::Rotate { .. }) if !resting => {
                                self.mode = mode;
                                self.rotate(working, time, out);
                            }
                            Some(mode) if !resting => {
                                self.mode = mode;
                                self.pinch(working, time, out);
                            }
                            _ if resting || most >= TWIST_GIVE_UP => {
                                self.enter(Mode::Pointer, time);
                                let split = self.split;
                                self.point(largest(&split), 0, time, factor, out);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn twist(&self, working: &[Finger], a: (f32, f32), b: (f32, f32)) -> Option<Mode> {
        if distance(a.0, a.1).min(distance(b.0, b.1)) < PIVOT_MOVE {
            return None;
        }
        let (p, q) = (working[0], working[1]);
        let now = (q.x - p.x, q.y - p.y);
        let then = (now.0 - (b.0 - a.0), now.1 - (b.1 - a.1));
        let spread = distance(now.0, now.1) - distance(then.0, then.1);
        let twist = turn(direction(then.0, then.1), direction(now.0, now.1));
        let arc = twist.abs() * distance(now.0, now.1) / 2.0;
        if self.config.rotate.enabled && degrees(twist.abs()) >= ROTATE_START && arc >= spread.abs() {
            Some(Mode::Rotate { last: direction(then.0, then.1), turned: 0.0 })
        } else if self.config.pinch.enabled && spread.abs() >= PINCH_START && spread.abs() > arc {
            Some(Mode::Pinch { last: distance(then.0, then.1), spread: 0.0 })
        } else {
            None
        }
    }

    fn rotate(&mut self, working: &[Finger], time: u64, out: &mut Vec<Event>) {
        let Mode::Rotate { last, turned } = self.mode else { return };
        let (p, q) = (working[0], working[1]);
        let angle = direction(q.x - p.x, q.y - p.y);
        let delta = degrees(turn(last, angle));
        let rotate = self.config.rotate;
        let turned = self.slide(rotate.action, turned, delta, rotate.step, (0.0, delta * ROTATE_SCROLL), time, out);
        self.mode = Mode::Rotate { last: angle, turned };
    }

    fn pinch(&mut self, working: &[Finger], time: u64, out: &mut Vec<Event>) {
        let Mode::Pinch { last, spread } = self.mode else { return };
        let (p, q) = (working[0], working[1]);
        let gap = distance(q.x - p.x, q.y - p.y);
        let delta = gap - last;
        let pinch = self.config.pinch;
        let spread = self.slide(pinch.action, spread, delta, pinch.step, (0.0, -delta), time, out);
        self.mode = Mode::Pinch { last: gap, spread };
    }

    fn gather(&mut self, mean: (f32, f32), time: u64) -> (f32, f32) {
        let since = *self.pending_since.get_or_insert(time);
        if time.saturating_sub(since) > GATHER {
            self.pending = (0.0, 0.0);
            self.pending_since = Some(time);
        }
        self.pending = (self.pending.0 + mean.0, self.pending.1 + mean.1);
        self.pending
    }

    fn many_fingers(&mut self, count: usize, mean: (f32, f32), elapsed: u64, time: u64, out: &mut Vec<Event>) {
        let fingers = u8::try_from(count).unwrap_or(u8::MAX);
        let drag = self.config.drag;
        if drag.enabled && fingers == drag.fingers {
            if self.mode == Mode::Drag {
                self.point(mean, elapsed, time, 1.0, out);
                return;
            }
            let pending = self.gather(mean, time);
            if distance(pending.0, pending.1) >= drag.start {
                self.mode = Mode::Drag;
                if self.carry.is_none() {
                    out.push(Event::Press(Button::Left));
                    self.carry = Some(Carry { grace: None });
                }
                let pending = self.pending;
                self.point(pending, elapsed, time, 1.0, out);
            }
            return;
        }
        match self.mode {
            Mode::Swipe { fingers, total } => {
                self.mode = Mode::Swipe { fingers, total: (total.0 + mean.0, total.1 + mean.1) };
                out.push(Event::Swipe { fingers, phase: Phase::Update, dx: mean.0, dy: mean.1 });
            }
            Mode::Switcher { travel } => {
                let (count, rest) = steps(travel, mean.0, self.config.swipes.step);
                for _ in 0..count.unsigned_abs() {
                    switch(count > 0, out);
                }
                self.mode = Mode::Switcher { travel: rest };
            }
            _ => {
                let pending = self.gather(mean, time);
                if distance(pending.0, pending.1) >= self.config.pointer.swipe_start {
                    let swipes = self.config.swipes;
                    if swipes.enabled && swipes.switcher == fingers && pending.0.abs() >= pending.1.abs() {
                        out.push(Event::Key { key: Key::Alt, pressed: true });
                        switch(pending.0 > 0.0, out);
                        self.mode = Mode::Switcher { travel: 0.0 };
                    } else {
                        self.mode = Mode::Swipe { fingers, total: pending };
                        out.push(Event::Swipe { fingers, phase: Phase::Begin, dx: pending.0, dy: pending.1 });
                    }
                }
            }
        }
    }

    fn gain(&self, millimetres: f32, elapsed: u64) -> f32 {
        let s = &self.config.pointer;
        if elapsed == 0 || elapsed > s.gap {
            return s.speed;
        }
        let velocity = millimetres * 1_000_000.0 / elapsed as f32;
        let ramp = ((velocity - s.slow) / (s.fast - s.slow)).clamp(0.0, 1.0);
        s.speed * (1.0 + (s.acceleration - 1.0) * ramp)
    }

    fn point(&mut self, step: (f32, f32), elapsed: u64, time: u64, factor: f32, out: &mut Vec<Event>) {
        if step.0 == 0.0 && step.1 == 0.0 {
            return;
        }
        let travel = distance(step.0, step.1);
        let gain = self.gain(travel, elapsed) * factor;
        let motion = (step.0 * gain, step.1 * gain);
        out.push(Event::Motion { dx: motion.0, dy: motion.1 });
        self.recent.push(time, elapsed, motion.0, motion.1, travel);
        self.mode_travel += travel;
        self.shake(step, time, out);
    }

    fn scroll(&mut self, (dx, dy): (f32, f32), elapsed: u64, time: u64, out: &mut Vec<Event>) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let sign = if self.config.pointer.natural_scroll { -1.0 } else { 1.0 };
        let speed = self.config.pointer.scroll_speed;
        let delta = (sign * dx * speed, sign * dy * speed);
        out.push(Event::Scroll { dx: delta.0, dy: delta.1 });
        self.recent.push(time, elapsed, delta.0, delta.1, distance(dx, dy));
        self.mode_travel += distance(dx, dy);
    }

    fn shake(&mut self, step: (f32, f32), time: u64, out: &mut Vec<Event>) {
        let shake = self.config.shake;
        let dragging = self.held.is_some() || self.carry.is_some() || self.tap_drag != TapDrag::Idle;
        if !shake.enabled || step.0 == 0.0 || dragging {
            self.shaking = Shaking::default();
            return;
        }
        let sign: i8 = if step.0 > 0.0 { 1 } else { -1 };
        let shaking = &mut self.shaking;
        if sign == shaking.sign {
            shaking.leg += step.0.abs();
            shaking.across += step.1.abs();
            return;
        }
        if shaking.leg >= shake.amplitude && shaking.leg >= 2.0 * shaking.across {
            if shaking.count == REVERSALS {
                shaking.reversals.copy_within(1.., 0);
                shaking.count -= 1;
            }
            shaking.reversals[shaking.count] = time;
            shaking.count += 1;
        }
        shaking.sign = sign;
        shaking.leg = step.0.abs();
        shaking.across = step.1.abs();
        let recent = shaking.reversals[..shaking.count].iter().filter(|at| time.saturating_sub(**at) <= shake.window).count();
        if recent >= usize::from(shake.turns.max(1)) {
            *shaking = Shaking::default();
            if let Some(action) = shake.action {
                self.act(action, time, out);
            }
        }
    }

    fn end_mode(&mut self, time: u64, out: &mut Vec<Event>) {
        match self.mode {
            Mode::Swipe { fingers, total } => {
                out.push(Event::Swipe { fingers, phase: Phase::End, dx: 0.0, dy: 0.0 });
                let swipes = self.config.swipes;
                if swipes.enabled {
                    let directions = if fingers >= 4 { swipes.four } else { swipes.three };
                    if let Some(action) = directions.toward(total.0, total.1) {
                        self.act(action, time, out);
                    }
                }
            }
            Mode::Switcher { .. } => out.push(Event::Key { key: Key::Alt, pressed: false }),
            Mode::Scroll if self.config.inertia.enabled && self.settled(time, 2) => self.remember_fling(Glide::Scroll, self.config.inertia.start, time),
            Mode::Pointer if self.config.throw.enabled && self.fling.is_none() && self.settled(time, 1) => {
                self.remember_fling(Glide::Pointer, self.config.throw.start, time)
            }
            _ => {}
        }
        self.mode = Mode::Idle;
        self.mode_travel = 0.0;
        self.pending = (0.0, 0.0);
        self.pending_since = None;
        self.split = [(0.0, 0.0); 2];
        self.recent.clear();
    }

    fn settled(&self, time: u64, fingers: u8) -> bool {
        self.sequence.is_some_and(|sequence| sequence.most <= fingers) && time.saturating_sub(self.mode_began) >= SETTLED && self.mode_travel >= FLUNG
    }

    fn remember_fling(&mut self, glide: Glide, start: f32, time: u64) {
        if let Some(velocity) = self.recent.velocity(time)
            && velocity.speed >= start
        {
            self.fling = Some(Fling { glide, velocity, ended: time });
        }
    }

    fn buttons(&mut self, button: bool, fingers: &[Finger], working: &[Finger], time: u64, out: &mut Vec<Event>) {
        match (button, self.held) {
            (true, None) => {
                let blocked = matches!(self.claim, Claim::Consumed | Claim::Rail { .. } | Claim::Circle { .. } | Claim::Symbol { .. })
                    || self.carry.is_some()
                    || self.tap_drag == TapDrag::Dragging;
                let ignored = blocked || fingers.is_empty() || fingers.iter().all(|finger| self.on_edge(finger, time));
                if let Some(sequence) = self.sequence.as_mut() {
                    sequence.clicked = true;
                }
                if ignored {
                    self.held = Some(Held::Ignored);
                } else {
                    let button = self.pressed(fingers, working);
                    self.held = Some(Held::Button(button));
                    self.held_ours = !self.native || matches!(self.claim, Claim::Joystick { .. });
                    if self.held_ours {
                        out.push(Event::Press(button));
                    }
                }
            }
            (false, Some(held)) => {
                self.held = None;
                if let Held::Button(button) = held
                    && core::mem::take(&mut self.held_ours)
                {
                    out.push(Event::Release(button));
                }
            }
            _ => {}
        }
    }

    fn pressed(&self, fingers: &[Finger], working: &[Finger]) -> Button {
        let areas = self.config.areas;
        if areas.enabled {
            let right = fingers.iter().any(|finger| finger.x >= self.surface.width * (1.0 - areas.width) && finger.y >= self.surface.height - areas.height);
            return if right { Button::Right } else { Button::Left };
        }
        by_fingers(working.len())
    }

    fn lifted(&mut self, time: u64, out: &mut Vec<Event>) {
        self.end_mode(time, out);
        if matches!(self.claim, Claim::Symbol { .. })
            && let Claim::Symbol { points, .. } = core::mem::replace(&mut self.claim, Claim::Consumed)
        {
            if stroke_length(&points) < self.config.pointer.tap_travel {
                out.push(Event::Notice(Notice::Cancelled));
                if let Some(sequence) = self.sequence.as_mut() {
                    sequence.claimed = false;
                }
            } else {
                self.recognize(&points, time, out);
            }
        }
        if let Some(sequence) = self.sequence.take() {
            let s = &self.config.pointer;
            let limit = if sequence.most > 1 { s.multi_tap_time } else { s.tap_time };
            let quick = time.saturating_sub(sequence.started) <= limit;
            let covering = matches!(self.covering, Some(Covering::Holding { .. }));
            let still =
                quick && !sequence.clicked && sequence.travelled <= s.tap_travel && self.held.is_none() && !sequence.claimed && !sequence.palmed && !covering;
            let tap = s.tap && still;
            if self.native {
                if still && sequence.hidden {
                    self.tapped(Tap { fingers: sequence.most, x: sequence.origin.0, y: sequence.origin.1, at: time }, time, out);
                }
            } else if self.tap_drag == TapDrag::Dragging {
                self.tap_drag = TapDrag::Idle;
                out.push(Event::Release(Button::Left));
                if tap && !self.locked {
                    out.push(Event::Press(Button::Left));
                    out.push(Event::Release(Button::Left));
                }
            } else if tap && self.carry.is_some() {
                self.carry = None;
                out.push(Event::Release(Button::Left));
            } else if tap {
                self.tapped(Tap { fingers: sequence.most, x: sequence.origin.0, y: sequence.origin.1, at: time }, time, out);
            }
        }
        if let Some(carry) = self.carry.as_mut()
            && carry.grace.is_none()
        {
            carry.grace = Some(time.saturating_add(self.config.drag.grace));
        }
        if let Some(fling) = self.fling.take()
            && time.saturating_sub(fling.ended) <= LIFT_GRACE
            && !self.locked
        {
            let (friction, stop) = match fling.glide {
                Glide::Scroll => (self.config.inertia.friction, self.config.inertia.stop * self.config.pointer.scroll_speed),
                Glide::Pointer => (self.config.throw.friction, self.config.throw.stop * self.config.pointer.speed),
            };
            self.kinetic = Some(Kinetic::new(fling.glide, fling.velocity, time, friction, stop));
        }
        self.anchor = None;
        self.pushed = None;
        if self.fingers.is_empty() {
            self.claim = Claim::None;
            self.covering = None;
        }
    }

    fn queue_candidate(&self, time: u64) -> bool {
        let Some(sequence) = self.sequence else { return true };
        let s = &self.config.pointer;
        sequence.most == 1
            && !sequence.clicked
            && !sequence.claimed
            && sequence.travelled <= s.tap_travel
            && time.saturating_sub(sequence.started) <= s.tap_time
    }

    fn flush(&mut self, time: u64, out: &mut Vec<Event>) {
        let queue = core::mem::take(&mut self.queue);
        if queue.is_empty() {
            return;
        }
        match self.patterns.exact(&queue, unlocking(self.locked)) {
            Some(action) => self.act(action, time, out),
            None if !self.locked => {
                for tap in &queue {
                    if self.native {
                        out.push(Event::Touch { fingers: tap.fingers, x: tap.x, y: tap.y });
                    } else if let Some(action) = self.config.taps.binding(tap.fingers) {
                        self.act(action, time, out);
                    }
                }
            }
            None => {}
        }
    }

    fn tapped(&mut self, tap: Tap, time: u64, out: &mut Vec<Event>) {
        if self.numpad && tap.fingers == 1 && !self.locked {
            self.flush(time, out);
            self.numpad_key(tap.x, tap.y, out);
            return;
        }
        if tap.fingers == 1 && !self.numpad && !self.patterns.is_empty() {
            let allow = unlocking(self.locked);
            let mut candidate = self.queue.clone();
            candidate.push(tap);
            if !self.patterns.starts(&candidate, &allow) {
                self.flush(time, out);
                candidate = vec![tap];
            }
            if self.patterns.starts(&candidate, &allow) {
                if !self.patterns.longer(&candidate, &allow)
                    && let Some(action) = self.patterns.exact(&candidate, &allow)
                {
                    self.queue.clear();
                    self.act(action, time, out);
                } else {
                    self.queue = candidate;
                    self.queue_until = tap.at.saturating_add(self.window);
                }
                return;
            }
        } else {
            self.flush(time, out);
        }
        if self.locked {
            return;
        }
        if self.native {
            out.push(Event::Touch { fingers: tap.fingers, x: tap.x, y: tap.y });
            return;
        }
        match self.config.taps.binding(tap.fingers) {
            Some(Action::Click(Button::Left)) if self.config.taps.drag && tap.fingers == 1 => {
                out.push(Event::Press(Button::Left));
                self.tap_drag = TapDrag::Waiting { until: time.saturating_add(self.config.taps.drag_time) };
            }
            Some(action) => self.act(action, time, out),
            None => {}
        }
    }

    fn numpad_key(&self, x: f32, y: f32, out: &mut Vec<Event>) {
        let rows = &self.config.numpad.rows;
        if rows.is_empty() {
            return;
        }
        let row = &rows[((y / self.surface.height * rows.len() as f32) as usize).min(rows.len() - 1)];
        if row.is_empty() {
            return;
        }
        if let Some(combo) = row[((x / self.surface.width * row.len() as f32) as usize).min(row.len() - 1)] {
            press(combo, out);
        }
    }

    fn recognize(&mut self, points: &[(f32, f32)], time: u64, out: &mut Vec<Event>) {
        match self.recognizer.recognize(points) {
            Some((index, score)) if score >= self.config.symbols.threshold => {
                out.push(Event::Notice(Notice::Symbol(index)));
                let action = self.recognizer.name(index).and_then(|name| self.config.symbols.action(name));
                if let Some(action) = action {
                    self.act(action, time, out);
                }
            }
            _ => out.push(Event::Notice(Notice::Unrecognized)),
        }
    }

    fn act(&mut self, action: Action, time: u64, out: &mut Vec<Event>) {
        if !unlocking(self.locked)(&action) {
            return;
        }
        match action {
            Action::Keys(combo) => press(combo, out),
            Action::Click(button) => {
                out.push(Event::Press(button));
                out.push(Event::Release(button));
            }
            Action::Toggle(Toggle::Touchpad) => self.lock(!self.locked, time, out),
            Action::Toggle(Toggle::Numpad) => {
                if self.config.numpad.enabled {
                    self.numpad = !self.numpad;
                    out.push(Event::Notice(Notice::Numpad(self.numpad)));
                }
            }
            Action::Toggle(Toggle::Tablet) => {
                if self.config.tablet.enabled {
                    self.set_tablet(!self.tablet, time, out);
                }
            }
            Action::Draw => {
                if self.config.symbols.enabled {
                    self.drawing = Some(time.saturating_add(self.config.symbols.timeout));
                    out.push(Event::Notice(Notice::Drawing));
                }
            }
        }
    }

    fn let_go(&mut self, time: u64, out: &mut Vec<Event>) {
        self.end_mode(time, out);
        if let Some(Held::Button(button)) = self.held {
            if core::mem::take(&mut self.held_ours) {
                out.push(Event::Release(button));
            }
            self.held = Some(Held::Ignored);
        }
        if self.carry.take().is_some() {
            out.push(Event::Release(Button::Left));
        }
        if self.tap_drag != TapDrag::Idle {
            self.tap_drag = TapDrag::Idle;
            out.push(Event::Release(Button::Left));
        }
        self.kinetic = None;
        self.fling = None;
        self.pushed = None;
        self.anchor = None;
        self.covering = None;
        self.claim = Claim::Consumed;
        self.mark_claimed();
    }

    fn lock(&mut self, on: bool, time: u64, out: &mut Vec<Event>) {
        if on == self.locked {
            return;
        }
        self.let_go(time, out);
        if on && self.tablet {
            self.set_tablet(false, time, out);
        }
        self.locked = on;
        out.push(Event::Notice(Notice::Touchpad(!on)));
    }

    fn set_tablet(&mut self, on: bool, time: u64, out: &mut Vec<Event>) {
        if on == self.tablet {
            return;
        }
        if on {
            self.let_go(time, out);
        } else if self.pen {
            out.push(Event::PenAway);
            self.pen = false;
        }
        self.tablet = on;
        out.push(Event::Notice(Notice::Tablet(on)));
    }

    fn tablet_frame(&mut self, button: bool, time: u64, out: &mut Vec<Event>) {
        let fingers: Vec<Finger> = self.fingers.iter().copied().filter(|finger| !finger.palm).collect();
        let Some(first) = fingers.first().copied() else {
            if self.pen {
                out.push(Event::PenAway);
                self.pen = false;
            }
            self.sequence = None;
            if self.fingers.is_empty() {
                self.claim = Claim::None;
            }
            return;
        };
        if self.sequence.is_none() {
            self.claim = Claim::None;
            self.sequence = Some(Sequence {
                started: time,
                most: 0,
                travelled: 0.0,
                clicked: false,
                origin: (first.x, first.y),
                claimed: false,
                hidden: false,
                palmed: false,
            });
        }
        if let Some(sequence) = self.sequence.as_mut() {
            sequence.most = sequence.most.max(u8::try_from(fingers.len()).unwrap_or(u8::MAX));
            sequence.travelled = fingers.iter().fold(sequence.travelled, |most, finger| most.max(finger.travelled));
        }
        if fingers.len() >= 3 {
            self.set_tablet(false, time, out);
            self.claim = Claim::Consumed;
            self.mark_claimed();
            self.working = fingers.iter().map(|finger| finger.id).collect();
            return;
        }
        if self.claim == Claim::Consumed {
            return;
        }
        let down = button || self.config.tablet.draw_on_touch;
        out.push(Event::Pen { x: (first.x / self.surface.width).clamp(0.0, 1.0), y: (first.y / self.surface.height).clamp(0.0, 1.0), down });
        self.pen = true;
        self.check_hold(time, out);
    }

    fn hold_target(&self) -> Option<(Action, u64)> {
        let corners = &self.config.corners;
        if !corners.enabled {
            return None;
        }
        let sequence = self.sequence?;
        if sequence.most != 1 || sequence.claimed || sequence.clicked {
            return None;
        }
        let [finger] = self.fingers.as_slice() else { return None };
        let finger = *finger;
        if finger.palm || finger.displacement() > self.config.pointer.tap_travel {
            return None;
        }
        let action = corners.get(Corner::at(finger.x0, finger.y0, self.surface, corners.size)?).hold?;
        if !unlocking(self.locked)(&action) || (self.tablet && !matches!(action, Action::Toggle(_))) {
            return None;
        }
        Some((action, finger.landed.saturating_add(corners.hold_time)))
    }

    fn check_hold(&mut self, time: u64, out: &mut Vec<Event>) {
        if let Some((action, at)) = self.hold_target()
            && time >= at
        {
            self.mark_claimed();
            self.end_mode(time, out);
            self.claim = Claim::Consumed;
            self.act(action, time, out);
        }
    }

    fn cover_at(&self) -> Option<u64> {
        match self.covering? {
            Covering::Holding { since, .. } if self.config.cover.enabled => Some(since.saturating_add(self.config.cover.time)),
            _ => None,
        }
    }

    fn check_cover(&mut self, time: u64, out: &mut Vec<Event>) {
        let cover = self.config.cover;
        if !cover.enabled || matches!(self.covering, Some(Covering::Covered | Covering::Spoiled)) || self.fingers.is_empty() {
            return;
        }
        let typed = self.typed.is_some_and(|at| at.saturating_add(COVER_QUIET) > self.touched);
        let pointing = self.fingers.iter().any(|finger| !finger.palm && finger.displacement() > self.config.pointer.tap_travel);
        let late = !self.fingers.iter().any(|finger| finger.palm) && time.saturating_sub(self.touched) > COVER_LANDING;
        let busy = self.locked || self.tablet || self.claim == Claim::Consumed;
        if typed || pointing || late || busy {
            self.covering = Some(Covering::Spoiled);
            return;
        }
        let (width, height) = (self.surface.width, self.surface.height);
        let inside =
            |finger: &&Finger| (COVER_MARGIN..=width - COVER_MARGIN).contains(&finger.x0) && (COVER_MARGIN..=height - COVER_MARGIN).contains(&finger.y0);
        let enough = self.fingers.len() >= usize::from(cover.contacts.max(1));
        let Some(palm) = self.fingers.iter().filter(|finger| finger.palm).find(inside).copied().filter(|_| enough) else {
            self.covering = None;
            return;
        };
        let (since, id, at) = match self.covering {
            Some(Covering::Holding { since, id, at }) => match self.fingers.iter().find(|finger| finger.id == id) {
                Some(followed) if distance(followed.x - at.0, followed.y - at.1) <= COVER_DRIFT => (since, id, at),
                Some(followed) => (time, id, (followed.x, followed.y)),
                None => (since, palm.id, (palm.x, palm.y)),
            },
            _ => (time, palm.id, (palm.x, palm.y)),
        };
        if time < since.saturating_add(cover.time) {
            self.covering = Some(Covering::Holding { since, id, at });
            return;
        }
        self.covering = Some(Covering::Covered);
        self.mark_claimed();
        self.end_mode(time, out);
        self.claim = Claim::Consumed;
        if let Some(action) = cover.action {
            self.act(action, time, out);
        }
    }

    fn track_dwell(&mut self, time: u64) {
        let dwell = self.config.dwell;
        let mut fingers = self.fingers.iter().filter(|finger| !finger.palm);
        let single = match (fingers.next(), fingers.next()) {
            (Some(finger), None) => Some(*finger),
            _ => None,
        };
        let eligible =
            dwell.enabled && self.claim == Claim::None && self.held.is_none() && self.carry.is_none() && matches!(self.mode, Mode::Pointer | Mode::Idle);
        let (Some(finger), true) = (single, eligible) else {
            self.anchor = None;
            return;
        };
        match self.anchor.as_mut() {
            None => self.anchor = Some(Anchor { x: finger.x, y: finger.y, since: time, travel: 0.0, fired: false }),
            Some(anchor) => {
                let moved = distance(finger.x - anchor.x, finger.y - anchor.y);
                if moved > dwell.still {
                    *anchor = Anchor { x: finger.x, y: finger.y, since: time, travel: anchor.travel + moved, fired: false };
                }
            }
        }
    }

    fn dwell_at(&self) -> Option<u64> {
        let anchor = self.anchor?;
        let dwell = self.config.dwell;
        (dwell.enabled && !self.locked && !anchor.fired && anchor.travel >= dwell.travel).then(|| anchor.since.saturating_add(dwell.time))
    }

    fn check_dwell(&mut self, time: u64, out: &mut Vec<Event>) {
        if let Some(at) = self.dwell_at()
            && time >= at
            && !self.locked
        {
            if let Some(anchor) = self.anchor.as_mut() {
                anchor.fired = true;
                anchor.travel = 0.0;
            }
            out.push(Event::Press(Button::Left));
            out.push(Event::Release(Button::Left));
        }
    }

    fn push_velocity(&self) -> Option<(f32, f32)> {
        if self.locked || self.tablet {
            return None;
        }
        if let Claim::Joystick { id, center } = self.claim {
            let finger = self.fingers.iter().find(|finger| finger.id == id)?;
            let (dx, dy) = (finger.x - center.0, finger.y - center.1);
            let reach = distance(dx, dy);
            let joystick = self.config.joystick;
            if reach <= joystick.dead {
                return None;
            }
            let speed = (reach - joystick.dead) * joystick.speed;
            return Some((dx / reach * speed, dy / reach * speed));
        }
        let edge = self.config.edge_motion;
        let dragging = matches!(self.held, Some(Held::Button(_))) || self.carry.is_some() || self.tap_drag == TapDrag::Dragging;
        if !edge.enabled || !dragging {
            return None;
        }
        let working: Vec<&Finger> = self.fingers.iter().filter(|finger| self.working.contains(&finger.id)).collect();
        if working.is_empty() {
            return None;
        }
        let count = working.len() as f32;
        let (x, y) = (working.iter().map(|finger| finger.x).sum::<f32>() / count, working.iter().map(|finger| finger.y).sum::<f32>() / count);
        let toward = |position: f32, size: f32| {
            if position <= edge.width {
                -1.0
            } else if position >= size - edge.width {
                1.0
            } else {
                0.0
            }
        };
        let (horizontal, vertical) = (toward(x, self.surface.width), toward(y, self.surface.height));
        if horizontal == 0.0 && vertical == 0.0 {
            return None;
        }
        let length = distance(horizontal, vertical);
        Some((horizontal / length * edge.speed, vertical / length * edge.speed))
    }

    fn push(&mut self, time: u64, out: &mut Vec<Event>) {
        match self.push_velocity() {
            None => self.pushed = None,
            Some((vx, vy)) => {
                let since = self.pushed.unwrap_or(time);
                self.pushed = Some(time);
                let elapsed = seconds(time.saturating_sub(since));
                if elapsed > 0.0 {
                    let gain = self.config.pointer.speed;
                    out.push(Event::Motion { dx: vx * elapsed * gain, dy: vy * elapsed * gain });
                }
            }
        }
    }

    fn resting(&self, finger: &Finger, time: u64) -> bool {
        finger.displacement() < self.config.pointer.rest_travel && time.saturating_sub(finger.landed) >= self.config.pointer.rest_time
    }

    fn thumb(&self, finger: &Finger, time: u64, others: usize) -> bool {
        others > 0 && finger.y >= self.surface.height * self.config.pointer.thumb_zone && self.resting(finger, time)
    }

    fn on_edge(&self, finger: &Finger, time: u64) -> bool {
        let s = &self.config.pointer;
        finger.y >= self.surface.height - s.edge && finger.displacement() < s.rest_travel && time.saturating_sub(finger.landed) >= s.edge_rest_time
    }

    fn at_edge(&self, x: f32, y: f32) -> bool {
        let s = &self.config.pointer;
        x <= s.side_edge || x >= self.surface.width - s.side_edge || y >= self.surface.height - s.bottom_edge
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DEFAULT;
    use alloc::string::String;
    use core::f32::consts::PI;

    const STEP: u64 = 8_000;
    const START: u64 = 1_000_000;
    const PAD: Surface = Surface { width: 117.0, height: 72.0 };

    type Frames = Vec<(bool, Vec<Touch>)>;

    fn touch(id: u8, x: f32, y: f32) -> Touch {
        Touch { id, x, y, palm: false }
    }

    fn palm(id: u8, x: f32, y: f32) -> Touch {
        Touch { id, x, y, palm: true }
    }

    fn gestures() -> Gestures {
        Gestures::new(DEFAULT, PAD)
    }

    fn with(config: Config) -> Gestures {
        Gestures::new(config, PAD)
    }

    fn run(gestures: &mut Gestures, frames: &[(bool, Vec<Touch>)]) -> Vec<Event> {
        frames
            .iter()
            .enumerate()
            .flat_map(|(i, (button, touches))| gestures.feed(&Input { time: START + i as u64 * STEP, button: *button, touches: touches.clone() }))
            .collect()
    }

    fn settle(gestures: &mut Gestures, until: u64) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..10_000 {
            match gestures.deadline() {
                Some(at) if at <= until => events.extend(gestures.tick(at)),
                _ => return events,
            }
        }
        panic!("the deadline never moved on");
    }

    fn play(gestures: &mut Gestures, frames: &[(bool, Vec<Touch>)], from: u64) -> (Vec<Event>, u64) {
        let mut events = Vec::new();
        let mut time = from;
        for (button, touches) in frames {
            events.extend(settle(gestures, time.saturating_sub(1)));
            events.extend(gestures.feed(&Input { time, button: *button, touches: touches.clone() }));
            time += STEP;
        }
        (events, time)
    }

    fn slide(id: u8, from: (f32, f32), to: (f32, f32), frames: usize) -> Vec<Vec<Touch>> {
        (0..=frames)
            .map(|i| {
                let t = i as f32 / frames as f32;
                vec![touch(id, from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)]
            })
            .collect()
    }

    fn total(events: &[Event]) -> (f32, f32) {
        events.iter().fold((0.0, 0.0), |sum, event| match event {
            Event::Motion { dx, dy } => (sum.0 + dx, sum.1 + dy),
            _ => sum,
        })
    }

    fn scrolled(events: &[Event]) -> (f32, f32) {
        events.iter().fold((0.0, 0.0), |sum, event| match event {
            Event::Scroll { dx, dy } => (sum.0 + dx, sum.1 + dy),
            _ => sum,
        })
    }

    fn clicks(events: &[Event]) -> Vec<Event> {
        events.iter().copied().filter(|e| matches!(e, Event::Press(_) | Event::Release(_))).collect()
    }

    fn pressed_keys(events: &[Event]) -> Vec<Key> {
        events.iter().filter_map(|event| if let Event::Key { key, pressed: true } = event { Some(*key) } else { None }).collect()
    }

    fn notices(events: &[Event]) -> Vec<Notice> {
        events.iter().filter_map(|event| if let Event::Notice(notice) = event { Some(*notice) } else { None }).collect()
    }

    fn still(frames: usize, touches: Vec<Touch>) -> Frames {
        (0..frames).map(|_| (false, touches.clone())).collect()
    }

    fn tap(x: f32, y: f32) -> Frames {
        vec![(false, vec![touch(1, x, y)]), (false, vec![touch(1, x + 0.2, y + 0.1)]), (false, vec![])]
    }

    fn lifted(frames: Vec<Vec<Touch>>) -> Frames {
        frames.into_iter().map(|touches| (false, touches)).chain([(false, vec![])]).collect()
    }

    fn action(text: &str) -> Option<Action> {
        Action::parse(text)
    }

    #[test]
    fn one_finger_moves_the_pointer_and_faster_moves_go_further() {
        let slow_events = run(&mut gestures(), &lifted(slide(1, (10.0, 10.0), (20.0, 10.0), 100)));
        let (dx, dy) = total(&slow_events);
        assert!((dx - 40.0).abs() < 0.5 && dy.abs() < 0.01, "{dx} {dy}");
        assert!(clicks(&slow_events).is_empty());
        let (fast_dx, _) = total(&run(&mut gestures(), &lifted(slide(1, (10.0, 10.0), (20.0, 10.0), 4))));
        assert!(fast_dx > 100.0, "{fast_dx}");
    }

    #[test]
    fn a_quick_still_touch_is_a_click_and_a_long_one_is_not() {
        assert_eq!(clicks(&run(&mut gestures(), &tap(30.0, 30.0))), [Event::Press(Button::Left), Event::Release(Button::Left)]);
        let mut held = still(40, vec![touch(2, 30.0, 30.0)]);
        held.push((false, vec![]));
        assert!(clicks(&run(&mut gestures(), &held)).is_empty());
        let mut off = DEFAULT;
        off.tap = false;
        assert!(clicks(&run(&mut Gestures::new(off, PAD), &tap(30.0, 30.0))).is_empty());
    }

    #[test]
    fn several_fingers_may_tap_a_little_slower_than_one() {
        let mut three = still(35, vec![touch(1, 30.0, 30.0), touch(2, 50.0, 30.0), touch(3, 70.0, 30.0)]);
        three.push((false, vec![]));
        assert_eq!(clicks(&run(&mut gestures(), &three)), [Event::Press(Button::Middle), Event::Release(Button::Middle)]);
        let mut one = still(35, vec![touch(1, 30.0, 30.0)]);
        one.push((false, vec![]));
        assert!(clicks(&run(&mut gestures(), &one)).is_empty());
        let two = [(false, vec![touch(1, 30.0, 30.0), touch(2, 50.0, 30.0)]), (false, vec![touch(1, 30.0, 30.0), touch(2, 50.0, 30.0)]), (false, vec![])];
        assert_eq!(clicks(&run(&mut gestures(), &two)), [Event::Press(Button::Right), Event::Release(Button::Right)]);
    }

    #[test]
    fn pressing_the_pad_clicks_by_the_number_of_fingers_and_is_not_also_a_tap() {
        let one = [(false, vec![touch(1, 30.0, 60.0)]), (true, vec![touch(1, 30.0, 60.0)]), (false, vec![touch(1, 30.0, 60.0)]), (false, vec![])];
        assert_eq!(clicks(&run(&mut gestures(), &one)), [Event::Press(Button::Left), Event::Release(Button::Left)]);
        let two = [(true, vec![touch(1, 30.0, 40.0), touch(2, 50.0, 40.0)]), (false, vec![touch(1, 30.0, 40.0), touch(2, 50.0, 40.0)]), (false, vec![])];
        assert_eq!(clicks(&run(&mut gestures(), &two)), [Event::Press(Button::Right), Event::Release(Button::Right)]);
    }

    #[test]
    fn two_fingers_moving_together_scroll_and_do_not_move_the_pointer() {
        let frames: Frames = (0..=30).map(|i| (false, vec![touch(1, 40.0, 20.0 + i as f32), touch(2, 60.0, 20.0 + i as f32)])).collect();
        let events = run(&mut gestures(), &frames);
        assert_eq!(total(&events), (0.0, 0.0));
        assert!((scrolled(&events).1 - 30.0 * DEFAULT.scroll_speed).abs() < 0.5, "{:?}", scrolled(&events));
        let mut natural = DEFAULT;
        natural.natural_scroll = true;
        let flipped = scrolled(&run(&mut Gestures::new(natural, PAD), &frames)).1;
        assert!((flipped + 30.0 * DEFAULT.scroll_speed).abs() < 0.5, "{flipped}");
    }

    #[test]
    fn three_fingers_swipe_with_a_beginning_and_an_end() {
        let frames =
            lifted((0..=20).map(|i| vec![touch(1, 30.0 + i as f32, 30.0), touch(2, 50.0 + i as f32, 30.0), touch(3, 70.0 + i as f32, 30.0)]).collect());
        let events = run(&mut gestures(), &frames);
        let phases: Vec<Phase> = events.iter().filter_map(|e| if let Event::Swipe { phase, fingers: 3, .. } = e { Some(*phase) } else { None }).collect();
        assert_eq!(phases.first(), Some(&Phase::Begin));
        assert_eq!(phases.last(), Some(&Phase::End));
        assert!(phases.iter().filter(|p| **p == Phase::Update).count() > 10);
        assert!(clicks(&events).is_empty() && total(&events) == (0.0, 0.0));
    }

    #[test]
    fn a_resting_thumb_neither_turns_pointing_into_scrolling_nor_clicks_into_right_clicks() {
        let thumb = touch(9, 24.0, 62.0);
        let mut frames = still(45, vec![thumb]);
        frames.extend((0..=40).map(|i| (false, vec![thumb, touch(1, 30.0 + i as f32 * 0.5, 15.0)])));
        frames.push((true, vec![thumb, touch(1, 50.0, 15.0)]));
        frames.push((false, vec![thumb, touch(1, 50.0, 15.0)]));
        frames.push((false, vec![]));
        let events = run(&mut gestures(), &frames);
        assert_eq!(scrolled(&events), (0.0, 0.0));
        assert!(total(&events).0 > 50.0, "{:?}", total(&events));
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
    }

    #[test]
    fn a_palm_never_points_nor_clicks_even_after_it_looks_like_a_finger_again() {
        let mut frames: Frames = (0..10).map(|i| (false, vec![palm(4, 80.0 + i as f32, 36.0)])).collect();
        frames.extend((0..10).map(|i| (false, vec![touch(4, 90.0 + i as f32, 36.0)])));
        frames.push((true, vec![touch(4, 100.0, 36.0)]));
        frames.push((false, vec![touch(4, 100.0, 36.0)]));
        frames.push((false, vec![]));
        assert!(run(&mut gestures(), &frames).is_empty());
    }

    #[test]
    fn a_hand_resting_on_the_bottom_edge_does_not_click_when_it_presses_the_pad() {
        let resting = touch(1, 33.6, 71.8);
        let mut frames = still(80, vec![resting]);
        frames.push((true, vec![resting]));
        frames.push((false, vec![resting]));
        frames.push((false, vec![]));
        assert!(clicks(&run(&mut gestures(), &frames)).is_empty());
        let corner = touch(2, 116.0, 69.4);
        let quick = [(false, vec![corner]), (false, vec![corner]), (true, vec![corner]), (false, vec![corner]), (false, vec![])];
        assert_eq!(clicks(&run(&mut gestures(), &quick)), [Event::Press(Button::Left), Event::Release(Button::Left)]);
    }

    #[test]
    fn a_hand_resting_on_the_edges_does_not_point_until_it_comes_well_inside() {
        let edge_frames = lifted((0..30).map(|i| vec![touch(5, 116.6, 24.0 + i as f32 * 0.1)]).collect());
        assert!(run(&mut gestures(), &edge_frames).is_empty());
        let bottom = lifted((0..30).map(|i| vec![touch(6, 33.0 + i as f32 * 0.2, 71.5)]).collect());
        assert!(run(&mut gestures(), &bottom).is_empty());
        let inward: Frames = (0..=40).map(|i| (false, vec![touch(7, 116.0 - i as f32 * 0.5, 30.0)])).collect();
        let (dx, _) = total(&run(&mut gestures(), &inward));
        assert!(dx < -20.0, "{dx}");
    }

    #[test]
    fn one_still_finger_and_one_moving_finger_point_instead_of_scrolling() {
        let frames: Frames = (0..=30).map(|i| (false, vec![touch(1, 40.0, 30.0), touch(2, 70.0, 20.0 + i as f32 * 0.5)])).collect();
        let events = run(&mut gestures(), &frames);
        assert_eq!(scrolled(&events), (0.0, 0.0));
        assert!(total(&events).1 > 30.0, "{:?}", total(&events));
        let mut config = Config::default();
        config.sniper.enabled = true;
        let careful = total(&run(&mut with(config), &frames)).1;
        let ratio = careful / total(&events).1;
        assert!((0.3..0.45).contains(&ratio), "{ratio}");
    }

    #[test]
    fn a_landing_finger_does_not_make_the_pointer_jump() {
        let landing = [(false, vec![touch(1, 10.0, 10.0)]), (false, vec![touch(1, 11.0, 10.0)]), (false, vec![touch(1, 11.0, 10.0), touch(2, 90.0, 40.0)])];
        let (dx, dy) = total(&run(&mut gestures(), &landing));
        assert!(dx < 10.0 && dy.abs() < 0.01, "{dx} {dy}");
    }

    #[test]
    fn nothing_waits_on_a_timer_unless_a_feature_needs_it() {
        let mut idle = gestures();
        run(&mut idle, &lifted(slide(1, (10.0, 10.0), (60.0, 10.0), 10)));
        assert_eq!(idle.deadline(), None);
    }

    fn flick(gestures: &mut Gestures, step: f32) -> (Vec<Event>, u64) {
        play(gestures, &lifted((0..=20).map(|i| vec![touch(1, 40.0, 10.0 + i as f32 * step), touch(2, 60.0, 10.0 + i as f32 * step)]).collect()), START)
    }

    #[test]
    fn a_flicked_scroll_glides_on_and_slows_down_and_a_touch_stops_it() {
        let mut config = Config::default();
        config.inertia.enabled = true;
        let mut flung = with(config.clone());
        let (during, end) = flick(&mut flung, 1.5);
        assert!(flung.deadline().is_some());
        let glide = settle(&mut flung, end + 5_000_000);
        let (moved, coasted) = (scrolled(&during).1, scrolled(&glide).1);
        assert!(coasted > moved, "{moved} {coasted}");
        let steps: Vec<f32> = glide.iter().filter_map(|e| if let Event::Scroll { dy, .. } = e { Some(*dy) } else { None }).collect();
        assert!(steps.windows(2).all(|pair| pair[1] <= pair[0] + 1e-3), "{steps:?}");
        assert_eq!(flung.deadline(), None);

        let mut caught = with(config.clone());
        let (_, end) = flick(&mut caught, 1.5);
        let early = settle(&mut caught, end + 100_000);
        caught.feed(&Input { time: end + 100_000, button: false, touches: vec![touch(5, 50.0, 30.0)] });
        assert!(!early.is_empty());
        assert_eq!(caught.deadline(), None);

        let mut slow = with(config);
        let (_, end) = flick(&mut slow, 0.3);
        assert!(settle(&mut slow, end + 1_000_000).is_empty());
    }

    #[test]
    fn the_end_of_a_three_finger_swipe_does_not_set_a_scroll_gliding() {
        let mut config = Config::default();
        config.inertia.enabled = true;
        let mut frames: Frames = (0..=20)
            .map(|i| (false, vec![touch(1, 30.0, 10.0 + i as f32 * 2.0), touch(2, 50.0, 10.0 + i as f32 * 2.0), touch(3, 70.0, 10.0 + i as f32 * 2.0)]))
            .collect();
        frames.extend((21..=23).map(|i| (false, vec![touch(1, 30.0, 10.0 + i as f32 * 2.0), touch(2, 50.0, 10.0 + i as f32 * 2.0)])));
        frames.push((false, vec![]));
        let mut pad = with(config);
        let (_, end) = play(&mut pad, &frames, START);
        assert_eq!(scrolled(&settle(&mut pad, end + 2_000_000)), (0.0, 0.0));
    }

    #[test]
    fn a_flicked_pointer_is_thrown_further() {
        let mut config = Config::default();
        config.throw.enabled = true;
        let mut thrown = with(config);
        let (_, end) = play(&mut thrown, &lifted(slide(1, (10.0, 30.0), (50.0, 30.0), 10)), START);
        let after = settle(&mut thrown, end + 2_000_000);
        assert!(total(&after).0 > 20.0, "{:?}", total(&after));
    }

    #[test]
    fn a_double_tap_in_the_corner_turns_the_touchpad_off_and_on() {
        let mut config = Config::default();
        config.corners.enabled = true;
        config.corners.top_left.double_tap = action("toggle-touchpad");
        let mut pad = with(config);
        let mut frames = tap(5.0, 5.0);
        frames.extend(tap(5.5, 5.2));
        let (events, mut time) = play(&mut pad, &frames, START);
        assert_eq!(notices(&events), [Notice::Touchpad(false)]);
        assert!(clicks(&events).is_empty());
        assert!(pad.locked());
        let mut ignored = lifted(slide(1, (40.0, 30.0), (80.0, 30.0), 20));
        ignored.extend(tap(60.0, 30.0));
        ignored.push((true, vec![touch(1, 60.0, 30.0)]));
        ignored.push((false, vec![]));
        let (events, after) = play(&mut pad, &ignored, time);
        time = after;
        let events = [events, settle(&mut pad, time + 1_000_000)].concat();
        assert!(events.is_empty(), "{events:?}");
        let (events, after) = play(&mut pad, &frames, time + 1_000_000);
        assert_eq!(notices(&events), [Notice::Touchpad(true)]);
        let (events, _) = play(&mut pad, &tap(60.0, 30.0), after);
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
    }

    #[test]
    fn turning_the_touchpad_off_leaves_no_timer_behind() {
        let mut config = Config::default();
        config.dwell.enabled = true;
        config.shake.enabled = true;
        config.shake.action = action("toggle-touchpad");
        let mut pad = with(config);
        let mut frames: Frames = wiggle(7, 12, 0.7).into_iter().map(|touches| (false, touches)).collect();
        let x = match frames.last() {
            Some((_, touches)) => touches[0].x,
            None => unreachable!(),
        };
        frames.extend(still(150, vec![touch(1, x, 30.0)]));
        let (events, end) = play(&mut pad, &frames, START);
        assert_eq!(notices(&events), [Notice::Touchpad(false)]);
        assert!(clicks(&events).is_empty());
        assert!(pad.deadline().is_none_or(|at| at > end), "{:?}", pad.deadline());
        assert!(settle(&mut pad, end + 5_000_000).is_empty());
    }

    #[test]
    fn touches_that_start_while_typing_are_ignored_and_earlier_ones_go_on() {
        let mut config = Config::default();
        config.typing.enabled = true;
        let mut pad = with(config);
        pad.typed(START - 100_000);
        let mut brushed = lifted(slide(1, (40.0, 40.0), (60.0, 40.0), 10));
        brushed.extend(tap(50.0, 30.0));
        let (events, time) = play(&mut pad, &brushed, START);
        assert!(events.is_empty(), "{events:?}");
        let (events, time) = play(&mut pad, &tap(50.0, 30.0), time + 600_000);
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
        let pointing: Frames = slide(1, (40.0, 40.0), (60.0, 40.0), 20).into_iter().map(|t| (false, t)).collect();
        let (first, middle) = play(&mut pad, &pointing[..10], time);
        pad.typed(middle);
        let (rest, _) = play(&mut pad, &pointing[10..], middle);
        assert!(total(&first).0 > 0.0 && total(&rest).0 > 0.0);
        let mut off = gestures();
        off.typed(START - 100_000);
        assert_eq!(clicks(&play(&mut off, &tap(50.0, 30.0), START).0).len(), 2);
    }

    fn native(config: Config) -> Gestures {
        Gestures::new(config, PAD).native()
    }

    fn watch(gestures: &mut Gestures, frames: &[(bool, Vec<Touch>)], from: u64) -> (Vec<Event>, Vec<usize>, u64) {
        let mut events = Vec::new();
        let mut shown = Vec::new();
        let mut time = from;
        for (button, touches) in frames {
            events.extend(settle(gestures, time.saturating_sub(1)));
            events.extend(gestures.feed(&Input { time, button: *button, touches: touches.clone() }));
            shown.push(gestures.visible().len());
            time += STEP;
        }
        (events, shown, time)
    }

    #[test]
    fn in_native_mode_the_usual_gestures_are_left_to_the_system_and_every_touch_is_shown() {
        let mut pad = native(Config::default());
        let mut frames = lifted(slide(1, (30.0, 30.0), (60.0, 30.0), 20));
        frames.extend(tap(50.0, 30.0));
        frames.extend([(false, vec![touch(1, 40.0, 30.0), touch(2, 60.0, 30.0)]), (true, vec![touch(1, 40.0, 30.0), touch(2, 60.0, 30.0)]), (false, vec![])]);
        frames.extend(lifted((0..=20).map(|i| vec![touch(1, 40.0, 20.0 + i as f32), touch(2, 60.0, 20.0 + i as f32)]).collect()));
        let (events, shown, _) = watch(&mut pad, &frames, START);
        assert!(events.is_empty(), "{events:?}");
        let expected: Vec<usize> = frames.iter().map(|(_, touches)| touches.len()).collect();
        assert_eq!(shown, expected);
        assert!(!pad.hidden());
    }

    #[test]
    fn in_native_mode_corner_taps_are_held_back_and_handed_over_when_they_are_not_ours() {
        let mut config = Config::default();
        config.corners.enabled = true;
        config.corners.top_left.double_tap = action("toggle-touchpad");
        let mut pad = native(config);
        let (events, shown, time) = watch(&mut pad, &tap(5.0, 5.0), START);
        assert!(events.is_empty() && shown.iter().all(|count| *count == 0), "{events:?} {shown:?}");
        assert_eq!(settle(&mut pad, time + 1_000_000), [Event::Touch { fingers: 1, x: 5.0, y: 5.0 }]);
        let mut twice = tap(5.0, 5.0);
        twice.extend(tap(5.5, 5.2));
        let (events, shown, time) = watch(&mut pad, &twice, time + 1_000_000);
        assert_eq!(events, [Event::Notice(Notice::Touchpad(false))]);
        assert!(shown.iter().all(|count| *count == 0));
        let (events, shown, time) = watch(&mut pad, &lifted(slide(1, (40.0, 30.0), (80.0, 30.0), 20)), time);
        assert!(events.is_empty() && shown.iter().all(|count| *count == 0));
        let (events, _, time) = watch(&mut pad, &twice, time);
        assert_eq!(events, [Event::Notice(Notice::Touchpad(true))]);
        let (events, shown, _) = watch(&mut pad, &lifted(slide(1, (5.0, 5.0), (40.0, 30.0), 20)), time + 1_000_000);
        assert!(events.is_empty());
        assert_eq!(shown[0], 0);
        assert!(shown[2..shown.len() - 1].iter().all(|count| *count == 1), "{shown:?}");
    }

    #[test]
    fn in_native_mode_rails_and_the_numpad_take_their_fingers_away_from_the_system() {
        let mut config = Config::default();
        config.rails.enabled = true;
        config.rails.sides = Slider::preset("volume").unwrap();
        let hands = lifted((0..=40).map(|i| vec![touch(1, 4.0, 50.0 - i as f32 * 0.5), touch(2, 113.0, 50.0 - i as f32 * 0.5)]).collect());
        let (events, shown, _) = watch(&mut native(config), &hands, START);
        assert_eq!(pressed_keys(&events), [Key::VolumeUp; 5]);
        assert!(shown.iter().all(|count| *count == 0), "{shown:?}");
        let mut config = Config::default();
        config.corners.enabled = true;
        config.corners.top_right.hold = action("numpad");
        config.numpad.enabled = true;
        let mut pad = native(config);
        let mut hold = still(130, vec![touch(1, 110.0, 5.0)]);
        hold.push((false, vec![]));
        let (events, _, time) = watch(&mut pad, &hold, START);
        assert_eq!(notices(&events), [Notice::Numpad(true)]);
        let (events, shown, time) = watch(&mut pad, &tap(10.0, 8.0), time);
        assert_eq!(pressed_keys(&events), [Key::Digit7]);
        assert!(shown.iter().all(|count| *count == 0));
        let (events, shown, _) = watch(&mut pad, &lifted(slide(1, (40.0, 30.0), (70.0, 30.0), 20)), time);
        assert!(events.is_empty());
        assert!(shown[3..shown.len() - 1].iter().all(|count| *count == 1), "{shown:?}");
    }

    #[test]
    fn in_native_mode_a_quick_tap_the_system_never_saw_is_handed_over() {
        let mut config = Config::default();
        config.rails.enabled = true;
        config.rails.top_bottom = Slider::preset("brightness").unwrap();
        config.joystick.enabled = true;
        let mut pad = native(config);
        let (events, shown, time) = watch(&mut pad, &tap(60.0, 66.0), START);
        assert_eq!(events, [Event::Touch { fingers: 1, x: 60.0, y: 66.0 }]);
        assert!(shown.iter().all(|count| *count == 0));
        let (events, _, time) = watch(&mut pad, &tap(105.0, 60.0), time + 1_000_000);
        assert_eq!(events, [Event::Touch { fingers: 1, x: 105.0, y: 60.0 }]);
        let mut slow = still(30, vec![touch(1, 60.0, 66.0)]);
        slow.push((false, vec![]));
        let (events, shown, _) = watch(&mut pad, &slow, time + 1_000_000);
        assert!(events.is_empty() && shown.iter().all(|count| *count == 0));
    }

    #[test]
    fn in_native_mode_a_hidden_finger_is_shown_once_it_moves_or_presses_and_never_just_for_lying_still() {
        let mut config = Config::default();
        config.rails.enabled = true;
        config.rails.top_bottom = Slider::preset("brightness").unwrap();
        let resting = touch(1, 60.0, 66.0);
        let mut frames = still(40, vec![resting]);
        frames.push((false, vec![]));
        let (events, shown, _) = watch(&mut native(config.clone()), &frames, START);
        assert!(events.is_empty() && shown.iter().all(|count| *count == 0), "{events:?} {shown:?}");
        let press = [(false, vec![resting]), (true, vec![resting]), (true, vec![resting]), (false, vec![resting]), (false, vec![])];
        let (events, shown, _) = watch(&mut native(config.clone()), &press, START);
        assert!(events.is_empty());
        assert_eq!(shown, [0, 1, 1, 1, 0]);
        let moving = lifted(slide(1, (60.0, 66.0), (75.0, 50.0), 15));
        let (events, shown, _) = watch(&mut native(config), &moving, START);
        assert!(events.is_empty(), "{events:?}");
        assert_eq!(shown[0], 0);
        assert!(shown[2..shown.len() - 1].iter().all(|count| *count == 1), "{shown:?}");
        let mut config = Config::default();
        config.joystick.enabled = true;
        let corner = touch(1, 105.0, 60.0);
        let click = [(false, vec![corner]), (true, vec![corner]), (false, vec![corner]), (false, vec![])];
        let (events, shown, _) = watch(&mut native(config), &click, START);
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
        assert!(shown.iter().all(|count| *count == 0));
    }

    #[test]
    fn a_single_tap_in_a_watched_corner_still_clicks_a_moment_later() {
        let mut config = Config::default();
        config.corners.enabled = true;
        config.corners.top_left.double_tap = action("toggle-touchpad");
        let mut pad = with(config);
        let (events, time) = play(&mut pad, &tap(5.0, 5.0), START);
        assert!(clicks(&events).is_empty());
        assert_eq!(clicks(&settle(&mut pad, time + 1_000_000)), [Event::Press(Button::Left), Event::Release(Button::Left)]);
        let (events, time) = play(&mut pad, &tap(5.0, 5.0), time + 1_000_000);
        assert!(clicks(&events).is_empty());
        let mut frames = lifted(slide(1, (5.0, 5.0), (40.0, 30.0), 20));
        frames.insert(0, (false, vec![]));
        let (moved, _) = play(&mut pad, &frames, time);
        let first_motion = moved.iter().position(|e| matches!(e, Event::Motion { .. })).unwrap();
        let first_click = moved.iter().position(|e| matches!(e, Event::Press(_))).unwrap();
        assert!(first_click < first_motion);
        let (dx, dy) = total(&moved);
        assert!(dx > 120.0 && dy > 80.0, "{dx} {dy}");
    }

    #[test]
    fn a_knock_code_runs_its_action_and_a_wrong_one_is_just_taps() {
        let mut config = Config::default();
        config.knock.enabled = true;
        config.knock.codes = vec![(vec![1, 2, 1, 4], action("mute").unwrap())];
        let mut pad = with(config);
        let code: Frames = [(20.0, 20.0), (90.0, 20.0), (20.0, 20.0), (90.0, 50.0)].iter().flat_map(|(x, y)| tap(*x, *y)).collect();
        let (events, time) = play(&mut pad, &code, START);
        let events = [events, settle(&mut pad, time + 1_000_000)].concat();
        assert_eq!(pressed_keys(&events), [Key::Mute]);
        assert!(clicks(&events).is_empty());
        let wrong: Frames = [(20.0, 20.0), (20.0, 50.0)].iter().flat_map(|(x, y)| tap(*x, *y)).collect();
        let (events, time) = play(&mut pad, &wrong, time + 1_000_000);
        let events = [events, settle(&mut pad, time + 1_000_000)].concat();
        assert_eq!(clicks(&events).len(), 4);
        assert!(pressed_keys(&events).is_empty());
    }

    #[test]
    fn holding_a_corner_runs_its_action_and_the_numpad_types_digits() {
        let mut config = Config::default();
        config.corners.enabled = true;
        config.corners.top_right.hold = action("numpad");
        config.numpad.enabled = true;
        let mut pad = with(config);
        let mut hold = still(130, vec![touch(1, 110.0, 5.0)]);
        hold.push((false, vec![]));
        let (events, time) = play(&mut pad, &hold, START);
        assert_eq!(notices(&events), [Notice::Numpad(true)]);
        assert!(clicks(&events).is_empty());
        let (events, time) = play(&mut pad, &tap(10.0, 8.0), time);
        assert_eq!(pressed_keys(&events), [Key::Digit7]);
        let (events, time) = play(&mut pad, &tap(110.0, 60.0), time);
        assert_eq!(pressed_keys(&events), [Key::Enter]);
        let (events, _) = play(&mut pad, &hold, time);
        assert_eq!(notices(&events), [Notice::Numpad(false)]);
        let mut with_palm = still(130, vec![touch(1, 110.0, 5.0), palm(2, 60.0, 70.0)]);
        with_palm.push((false, vec![]));
        assert!(notices(&play(&mut pad, &with_palm, time + 5_000_000).0).is_empty());
    }

    #[test]
    fn a_resting_hand_creeping_slowly_neither_swipes_nor_drags() {
        let creep = lifted(
            (0..=150)
                .map(|i| vec![touch(1, 30.0, 40.0 - i as f32 * 0.08), touch(2, 50.0, 38.0 - i as f32 * 0.08), touch(3, 70.0, 40.0 - i as f32 * 0.08)])
                .collect(),
        );
        let events = run(&mut gestures(), &creep);
        assert!(!events.iter().any(|e| matches!(e, Event::Swipe { .. })), "{events:?}");
        let mut config = Config::default();
        config.drag.enabled = true;
        assert!(clicks(&run(&mut with(config), &creep)).is_empty());
    }

    #[test]
    fn three_fingers_drag_with_the_button_held_and_let_go_after_a_pause() {
        let mut config = Config::default();
        config.drag.enabled = true;
        let mut pad = with(config);
        let frames = lifted(
            (0..=20)
                .map(|i| vec![touch(1, 30.0 + i as f32 * 0.5, 30.0), touch(2, 50.0 + i as f32 * 0.5, 30.0), touch(3, 70.0 + i as f32 * 0.5, 30.0)])
                .collect(),
        );
        let (events, time) = play(&mut pad, &frames, START);
        assert_eq!(clicks(&events), [Event::Press(Button::Left)]);
        assert!(total(&events).0 > 20.0);
        assert!(!events.iter().any(|e| matches!(e, Event::Swipe { .. })));
        let (more, time) = play(&mut pad, &lifted(slide(4, (60.0, 30.0), (70.0, 30.0), 10)), time + 200_000);
        assert!(clicks(&more).is_empty() && total(&more).0 > 10.0);
        assert_eq!(clicks(&settle(&mut pad, time + 1_000_000)), [Event::Release(Button::Left)]);
    }

    #[test]
    fn swipes_run_actions_and_three_fingers_can_switch_windows() {
        let mut config = Config::default();
        config.swipes.enabled = true;
        config.swipes.three.left = action("forward");
        let mut pad = with(config.clone());
        let left = lifted((0..=15).map(|i| vec![touch(1, 60.0 - i as f32, 30.0), touch(2, 75.0 - i as f32, 30.0), touch(3, 90.0 - i as f32, 30.0)]).collect());
        assert_eq!(pressed_keys(&run(&mut pad, &left)), [Key::Alt, Key::Right]);
        config.swipes.switcher = 3;
        let mut switching = with(config);
        let right = lifted((0..=30).map(|i| vec![touch(1, 20.0 + i as f32, 30.0), touch(2, 35.0 + i as f32, 30.0), touch(3, 50.0 + i as f32, 30.0)]).collect());
        let events = run(&mut switching, &right);
        let keys: Vec<(Key, bool)> = events.iter().filter_map(|e| if let Event::Key { key, pressed } = e { Some((*key, *pressed)) } else { None }).collect();
        assert_eq!(keys.first(), Some(&(Key::Alt, true)));
        assert_eq!(keys.last(), Some(&(Key::Alt, false)));
        assert_eq!(pressed_keys(&events).iter().filter(|key| **key == Key::Tab).count(), 3);
    }

    #[test]
    fn two_hands_on_the_side_edges_change_the_volume() {
        let mut config = Config::default();
        config.rails.enabled = true;
        config.rails.sides = Slider::preset("volume").unwrap();
        let frames = lifted((0..=40).map(|i| vec![touch(1, 4.0, 50.0 - i as f32 * 0.5), touch(2, 113.0, 50.0 - i as f32 * 0.5)]).collect());
        let events = run(&mut with(config), &frames);
        assert_eq!(pressed_keys(&events), [Key::VolumeUp; 5]);
        assert_eq!(scrolled(&events), (0.0, 0.0));
        assert_eq!(total(&events), (0.0, 0.0));
        assert!(clicks(&events).is_empty());
    }

    #[test]
    fn one_finger_along_the_right_edge_scrolls_like_an_old_touchpad() {
        let mut config = Config::default();
        config.rails.enabled = true;
        config.rails.right = Slider::Scroll;
        let events = run(&mut with(config), &lifted(slide(1, (112.0, 20.0), (112.0, 35.0), 30)));
        assert_eq!(total(&events), (0.0, 0.0));
        assert!((scrolled(&events).1 - 15.0 * DEFAULT.scroll_speed).abs() < 1.0, "{:?}", scrolled(&events));
        let mut config = Config::default();
        config.rails.enabled = true;
        config.rails.right = Slider::Scroll;
        let across = run(&mut with(config), &lifted(slide(1, (112.0, 20.0), (90.0, 22.0), 30)));
        assert_eq!(scrolled(&across), (0.0, 0.0));
        assert!(total(&across).0 < -60.0, "{:?}", total(&across));
    }

    #[test]
    fn circling_along_the_edges_keeps_scrolling() {
        let mut config = Config::default();
        config.circular.enabled = true;
        let (cx, cy, rx, ry) = (58.5, 36.0, 53.5, 31.0);
        let frames = lifted(
            (0..=120)
                .map(|i| {
                    let angle = i as f32 / 120.0 * 2.0 * PI;
                    vec![touch(1, cx + rx * libm::cosf(angle), cy + ry * libm::sinf(angle))]
                })
                .collect(),
        );
        let events = run(&mut with(config), &frames);
        let expected = 90.0 * DEFAULT.scroll_speed;
        assert!((scrolled(&events).1 - expected).abs() < expected * 0.1, "{:?}", scrolled(&events));
        assert_eq!(total(&events), (0.0, 0.0));
    }

    #[test]
    fn turning_two_fingers_rotates_and_spreading_them_zooms() {
        let mut config = Config::default();
        config.rotate.enabled = true;
        config.rotate.action = Slider::preset("rotate").unwrap();
        config.pinch.enabled = true;
        let turning = lifted(
            (0..=30)
                .map(|i| {
                    let angle = i as f32 / 30.0 * PI / 2.0;
                    let (dx, dy) = (15.0 * libm::cosf(angle), 15.0 * libm::sinf(angle));
                    vec![touch(1, 58.0 - dx, 36.0 - dy), touch(2, 58.0 + dx, 36.0 + dy)]
                })
                .collect(),
        );
        let events = run(&mut with(config.clone()), &turning);
        assert_eq!(pressed_keys(&events).iter().filter(|key| **key == Key::R).count(), 3, "{:?}", pressed_keys(&events));
        assert!(!pressed_keys(&events).contains(&Key::Shift));
        let spreading = lifted((0..=30).map(|i| vec![touch(1, 48.0 - i as f32 * 0.5, 36.0), touch(2, 68.0 + i as f32 * 0.5, 36.0)]).collect());
        let events = run(&mut with(config), &spreading);
        assert_eq!(pressed_keys(&events).iter().filter(|key| **key == Key::KpPlus).count(), 5, "{:?}", pressed_keys(&events));
        assert_eq!(scrolled(&events), (0.0, 0.0));
    }

    #[test]
    fn a_rotation_around_a_finger_that_hardly_moves_still_rotates_but_a_resting_finger_means_pointing() {
        let mut config = Config::default();
        config.rotate.enabled = true;
        config.rotate.action = Slider::preset("rotate").unwrap();
        let pivot = (50.0, 33.0);
        let turning = lifted(
            (0..=45)
                .map(|i| {
                    let angle = (-90.0 + i as f32 * 3.0) * PI / 180.0;
                    let drift = (i.min(6) as f32) * 0.2;
                    vec![touch(1, pivot.0 + 27.0 * libm::cosf(angle), pivot.1 + 27.0 * libm::sinf(angle)), touch(2, pivot.0 - drift, pivot.1)]
                })
                .collect(),
        );
        let events = run(&mut with(config.clone()), &turning);
        assert_eq!(pressed_keys(&events).iter().filter(|key| **key == Key::R).count(), 4, "{:?}", pressed_keys(&events));
        assert_eq!(total(&events), (0.0, 0.0));
        let wide = lifted(
            (0..=60)
                .map(|i| {
                    let angle = (150.0 - i as f32 * 3.0) * PI / 180.0;
                    let gap = 54.0 - (i.min(10) as f32) * 0.5;
                    let (dx, dy) = (gap / 2.0 * libm::cosf(angle), gap / 2.0 * libm::sinf(angle));
                    vec![touch(1, 70.0 - dx, 32.0 - dy), touch(2, 70.0 + dx, 32.0 + dy)]
                })
                .collect(),
        );
        let events = run(&mut with(config.clone()), &wide);
        assert_eq!(pressed_keys(&events).iter().filter(|key| **key == Key::R).count(), 6, "{:?}", pressed_keys(&events));
        assert!(pressed_keys(&events).contains(&Key::Shift));
        assert_eq!(total(&events), (0.0, 0.0));
        let pointing: Frames = (0..=30).map(|i| (false, vec![touch(1, 40.0, 30.0), touch(2, 70.0, 20.0 + i as f32 * 0.5)])).collect();
        let events = run(&mut with(config), &pointing);
        assert!(pressed_keys(&events).is_empty());
        let (dx, dy) = total(&events);
        assert!(dy > 50.0 && dx.abs() < 1.0, "{dx} {dy}");
    }

    #[test]
    fn a_symbol_drawn_after_arming_runs_its_action() {
        let mut config = Config::default();
        config.symbols.enabled = true;
        config.taps.three = Some(Action::Draw);
        config.symbols.actions = vec![(String::from("circle"), action("calculator").unwrap())];
        let mut pad = with(config);
        let mut frames = still(3, vec![touch(1, 30.0, 30.0), touch(2, 50.0, 30.0), touch(3, 70.0, 30.0)]);
        frames.push((false, vec![]));
        frames.extend(lifted(
            (0..=60)
                .map(|i| {
                    let angle = i as f32 / 60.0 * 2.0 * PI - PI / 2.0;
                    vec![touch(4, 58.0 + 15.0 * libm::cosf(angle), 36.0 + 15.0 * libm::sinf(angle))]
                })
                .collect(),
        ));
        let (events, _) = play(&mut pad, &frames, START);
        let seen = notices(&events);
        assert_eq!(seen.first(), Some(&Notice::Drawing));
        let Some(Notice::Symbol(index)) = seen.get(1) else { panic!("{seen:?}") };
        assert_eq!(pad.symbol(*index), Some("circle"));
        assert_eq!(pressed_keys(&events), [Key::Calculator]);
        assert_eq!(total(&events), (0.0, 0.0));
        assert!(clicks(&events).is_empty());
    }

    #[test]
    fn a_tap_or_a_second_finger_right_after_arming_cancels_the_drawing() {
        let mut config = Config::default();
        config.symbols.enabled = true;
        config.taps.three = Some(Action::Draw);
        let mut arm = still(3, vec![touch(1, 30.0, 30.0), touch(2, 50.0, 30.0), touch(3, 70.0, 30.0)]);
        arm.push((false, vec![]));
        let mut frames = arm.clone();
        frames.extend([
            (false, vec![touch(4, 40.0, 30.0)]),
            (false, vec![touch(4, 40.0, 30.0), touch(5, 60.0, 30.0)]),
            (false, vec![touch(4, 40.0, 30.0), touch(5, 60.0, 30.0)]),
            (false, vec![]),
        ]);
        let (events, _) = play(&mut with(config.clone()), &frames, START);
        assert_eq!(notices(&events), [Notice::Drawing, Notice::Cancelled]);
        assert_eq!(clicks(&events), [Event::Press(Button::Right), Event::Release(Button::Right)]);
        let mut frames = arm;
        frames.extend(tap(40.0, 30.0));
        let (events, _) = play(&mut with(config), &frames, START);
        assert_eq!(notices(&events), [Notice::Drawing, Notice::Cancelled]);
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
    }

    #[test]
    fn a_finger_in_the_joystick_corner_keeps_the_pointer_going() {
        let mut config = Config::default();
        config.joystick.enabled = true;
        let mut pad = with(config);
        let mut frames: Frames = slide(1, (105.0, 60.0), (108.0, 60.0), 5).into_iter().map(|t| (false, t)).collect();
        frames.extend(still(20, vec![touch(1, 108.0, 60.0)]));
        let (events, _) = play(&mut pad, &frames, START);
        let (dx, dy) = total(&events);
        assert!(dx > 20.0 && dy.abs() < 0.01, "{dx} {dy}");
    }

    #[test]
    fn resting_after_a_move_clicks_once_for_those_who_cannot_press() {
        let mut config = Config::default();
        config.dwell.enabled = true;
        let mut pad = with(config);
        let mut frames: Frames = slide(1, (30.0, 30.0), (40.0, 30.0), 20).into_iter().map(|t| (false, t)).collect();
        frames.extend(still(150, vec![touch(1, 40.0, 30.0)]));
        let (events, _) = play(&mut pad, &frames, START);
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
    }

    #[test]
    fn covering_the_pad_with_a_hand_runs_its_action_once() {
        let mut config = Config::default();
        config.cover.enabled = true;
        let hand = vec![touch(1, 30.0, 30.0), palm(2, 55.0, 40.0), touch(3, 90.0, 35.0)];
        let mut frames = still(100, hand);
        frames.push((false, vec![]));
        let (events, _) = play(&mut with(config.clone()), &frames, START);
        assert_eq!(pressed_keys(&events), [Key::PlayPause]);
        assert!(clicks(&events).is_empty());
        let mut resting = still(100, vec![touch(1, 30.0, 30.0), touch(2, 50.0, 25.0), touch(3, 70.0, 28.0), touch(4, 90.0, 35.0)]);
        resting.push((false, vec![]));
        assert!(pressed_keys(&play(&mut with(config), &resting, START).0).is_empty());
    }

    fn settling_hand() -> Frames {
        lifted(
            (0..150)
                .map(|i| {
                    let drift = (i.min(100) as f32) * 0.2;
                    let mut hand = vec![palm(4, 64.0 + drift, 39.0 - drift / 2.0)];
                    if i % 20 < 6 {
                        hand.push(if i % 20 < 3 { touch(5, 112.0, 56.0) } else { palm(5, 112.0, 56.0) });
                    }
                    hand
                })
                .collect(),
        )
    }

    #[test]
    fn a_hand_the_pad_reports_as_one_shifting_palm_still_covers_it() {
        let mut config = Config::default();
        config.cover.enabled = true;
        let (events, _) = play(&mut with(config.clone()), &settling_hand(), START);
        assert_eq!(pressed_keys(&events), [Key::PlayPause]);
        assert!(clicks(&events).is_empty(), "{events:?}");
        let handed_over = lifted(
            (0..80)
                .map(|i| match i {
                    0..10 => vec![palm(4, 70.0, 35.0)],
                    10..40 => vec![palm(4, 70.0, 35.0), palm(5, 100.0, 50.0)],
                    _ => vec![palm(5, 100.0, 50.0)],
                })
                .collect(),
        );
        assert_eq!(pressed_keys(&play(&mut with(config.clone()), &handed_over, START).0), [Key::PlayPause]);
        let mut late = still(30, vec![touch(1, 47.0, 50.0)]);
        late.extend(settling_hand().into_iter().map(|(button, mut hand)| {
            hand.push(touch(1, 47.0, 50.0));
            (button, hand)
        }));
        late.push((false, vec![]));
        assert!(pressed_keys(&play(&mut with(config.clone()), &late, START).0).is_empty());
        let mut pressing = settling_hand();
        for frame in &mut pressing[46..59] {
            frame.0 = true;
        }
        let (events, _) = play(&mut with(config.clone()), &pressing, START);
        assert_eq!(pressed_keys(&events), [Key::PlayPause]);
        assert!(clicks(&events).is_empty(), "{events:?}");
        let beside = still(150, vec![palm(1, 113.9, 71.0), touch(2, 4.0, 3.9)]);
        assert!(pressed_keys(&play(&mut with(config.clone()), &beside, START).0).is_empty());
        let creeping = lifted((0..150).map(|i| vec![palm(1, 112.0 - (i.min(30) as f32) * 0.4, 40.0)]).collect());
        assert!(pressed_keys(&play(&mut with(config), &creeping, START).0).is_empty());
    }

    #[test]
    fn fingers_of_a_covering_hand_do_not_tap() {
        let mut config = Config::default();
        config.cover.enabled = true;
        let blip = |frames: core::ops::Range<usize>, i: usize| frames.contains(&i).then(|| touch(7, 108.0, 25.0));
        let hand = lifted((0..150).map(|i| [Some(palm(4, 68.0, 33.0)), blip(10..15, i).or(blip(90..100, i))].into_iter().flatten().collect()).collect());
        let (events, _) = play(&mut with(config.clone()), &hand, START);
        assert_eq!(pressed_keys(&events), [Key::PlayPause]);
        assert!(clicks(&events).is_empty(), "{events:?}");
        let (events, _) = play(&mut with(Config::default()), &hand, START);
        assert_eq!(clicks(&events).len(), 4, "{events:?}");
    }

    #[test]
    fn a_palm_that_lands_within_a_second_of_typing_never_covers_even_after_typing_stops() {
        let mut config = Config::default();
        assert!(!config.uses_typing(false));
        config.typing.enabled = true;
        assert!(config.uses_typing(false) && !config.uses_typing(true));
        config.typing.enabled = false;
        config.cover.enabled = true;
        assert!(config.uses_typing(false) && config.uses_typing(true));
        let landing = START + 2_000_000;
        for (typed, covers) in [(landing - 600_000, false), (landing - 1_100_000, true)] {
            let mut pad = with(config.clone());
            pad.typed(typed);
            assert_eq!(pressed_keys(&play(&mut pad, &settling_hand(), landing).0).len(), usize::from(covers));
            let mut native = Gestures::new(config.clone(), PAD).native();
            native.typed(typed);
            assert_eq!(pressed_keys(&play(&mut native, &settling_hand(), landing).0).len(), usize::from(covers));
        }
        let mut pad = with(config);
        let frames = settling_hand();
        let (_, middle) = play(&mut pad, &frames[..30], START);
        pad.typed(middle);
        assert!(pressed_keys(&play(&mut pad, &frames[30..], middle).0).is_empty());
    }

    #[test]
    fn a_finger_that_the_pad_later_flags_as_a_palm_is_not_a_tap() {
        let mut frames: Frames = (0..5).map(|_| (false, vec![touch(1, 112.0, 56.0)])).collect();
        frames.extend((0..20).map(|_| (false, vec![palm(1, 112.0, 56.0)])));
        frames.push((false, vec![]));
        let (events, _) = play(&mut gestures(), &frames, START);
        assert!(clicks(&events).is_empty(), "{events:?}");
        let (events, _) = play(&mut gestures(), &frames[..5].iter().cloned().chain([(false, vec![])]).collect::<Frames>(), START);
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
    }

    fn wiggle(legs: usize, frames: usize, step: f32) -> Vec<Vec<Touch>> {
        let mut wiggle = Vec::new();
        let mut x = 40.0;
        for leg in 0..legs {
            for _ in 0..frames {
                x += if leg % 2 == 0 { step } else { -step };
                wiggle.push(vec![touch(1, x, 30.0)]);
            }
        }
        wiggle
    }

    #[test]
    fn shaking_the_pointer_runs_its_action_but_not_while_dragging() {
        let mut config = Config::default();
        config.shake.enabled = true;
        let events = run(&mut with(config.clone()), &lifted(wiggle(7, 12, 0.7)));
        assert_eq!(pressed_keys(&events), [Key::Ctrl]);
        assert!(pressed_keys(&run(&mut with(config.clone()), &lifted(wiggle(7, 12, 0.3)))).is_empty());
        let dragging: Frames = wiggle(7, 12, 0.7).into_iter().map(|touches| (true, touches)).chain([(false, vec![])]).collect();
        assert!(pressed_keys(&run(&mut with(config.clone()), &dragging)).is_empty());
        let circles = lifted(
            (0..=150)
                .map(|i| {
                    let angle = i as f32 / 12.0 * 2.0 * PI;
                    vec![touch(1, 58.0 + 8.0 * libm::cosf(angle), 36.0 + 8.0 * libm::sinf(angle))]
                })
                .collect(),
        );
        assert!(pressed_keys(&run(&mut with(config), &circles)).is_empty());
    }

    #[test]
    fn the_tablet_mode_places_a_pen_and_three_fingers_leave_it() {
        let mut config = Config::default();
        config.tablet.enabled = true;
        config.taps.four = action("tablet");
        let mut pad = with(config);
        let mut frames = still(3, vec![touch(1, 30.0, 30.0), touch(2, 50.0, 30.0), touch(3, 70.0, 30.0), touch(4, 90.0, 30.0)]);
        frames.push((false, vec![]));
        frames.push((false, vec![touch(5, 58.5, 36.0)]));
        frames.push((true, vec![touch(5, 58.5, 36.0)]));
        frames.push((false, vec![]));
        frames.extend(still(3, vec![touch(6, 30.0, 30.0), touch(7, 50.0, 30.0), touch(8, 70.0, 30.0)]));
        frames.push((false, vec![]));
        let (events, _) = play(&mut pad, &frames, START);
        assert_eq!(notices(&events), [Notice::Tablet(true), Notice::Tablet(false)]);
        let pens: Vec<Event> = events.iter().copied().filter(|e| matches!(e, Event::Pen { .. } | Event::PenAway)).collect();
        assert_eq!(pens, [Event::Pen { x: 0.5, y: 0.5, down: false }, Event::Pen { x: 0.5, y: 0.5, down: true }, Event::PenAway]);
        assert!(clicks(&events).is_empty());
    }

    #[test]
    fn swiping_in_from_the_left_edge_goes_back() {
        let mut config = Config::default();
        config.edge_swipes.enabled = true;
        config.edge_swipes.left = action("back");
        let events = run(&mut with(config), &lifted(slide(1, (1.0, 36.0), (16.0, 37.0), 10)));
        assert_eq!(pressed_keys(&events), [Key::Alt, Key::Left]);
        assert_eq!(total(&events), (0.0, 0.0));
    }

    #[test]
    fn pressing_the_lower_right_area_is_a_right_click() {
        let mut config = Config::default();
        config.areas.enabled = true;
        let frames = [(false, vec![touch(1, 100.0, 65.0)]), (true, vec![touch(1, 100.0, 65.0)]), (false, vec![touch(1, 100.0, 65.0)]), (false, vec![])];
        assert_eq!(clicks(&run(&mut with(config.clone()), &frames)), [Event::Press(Button::Right), Event::Release(Button::Right)]);
        let left = [(true, vec![touch(1, 30.0, 65.0)]), (false, vec![touch(1, 30.0, 65.0)]), (false, vec![])];
        assert_eq!(clicks(&run(&mut with(config), &left)), [Event::Press(Button::Left), Event::Release(Button::Left)]);
    }

    #[test]
    fn tap_and_drag_holds_the_button_and_two_taps_are_a_double_click() {
        let mut config = Config::default();
        config.taps.drag = true;
        let mut pad = with(config.clone());
        let mut frames = tap(50.0, 30.0);
        frames.extend(lifted(slide(1, (50.0, 30.0), (70.0, 30.0), 10)));
        let (events, _) = play(&mut pad, &frames, START);
        assert_eq!(clicks(&events), [Event::Press(Button::Left), Event::Release(Button::Left)]);
        let release = events.iter().position(|e| *e == Event::Release(Button::Left)).unwrap();
        assert!(events[..release].iter().any(|e| matches!(e, Event::Motion { .. })));
        let mut twice = tap(50.0, 30.0);
        twice.extend(tap(50.0, 30.0));
        let (events, _) = play(&mut with(config.clone()), &twice, START);
        assert_eq!(clicks(&events).len(), 4);
        let mut once = with(config);
        let (events, time) = play(&mut once, &tap(50.0, 30.0), START);
        assert_eq!(clicks(&events), [Event::Press(Button::Left)]);
        assert_eq!(clicks(&settle(&mut once, time + 1_000_000)), [Event::Release(Button::Left)]);
    }

    #[test]
    fn dragging_against_an_edge_keeps_moving_that_way() {
        let mut config = Config::default();
        config.edge_motion.enabled = true;
        let mut frames: Frames = vec![(false, vec![touch(1, 100.0, 30.0)])];
        frames.extend(slide(1, (100.0, 30.0), (113.0, 30.0), 10).into_iter().map(|t| (true, t)));
        frames.extend((0..30).map(|_| (true, vec![touch(1, 113.0, 30.0)])));
        frames.push((false, vec![touch(1, 113.0, 30.0)]));
        frames.push((false, vec![]));
        let (events, _) = play(&mut with(config.clone()), &frames, START);
        let (plain, _) = play(&mut with(Config::default()), &frames, START);
        assert!(total(&events).0 > total(&plain).0 + 30.0, "{:?} {:?}", total(&events), total(&plain));
    }
}
