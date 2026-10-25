use crate::vec2::Vec2;

/// Outcome of a narrowphase test. Methods only fill in what they compute:
/// a boolean test leaves `distance`, `normal` and `points` empty.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    pub intersect: bool,
    /// Signed distance: positive gap when apart, negative penetration depth when overlapping.
    pub distance: Option<f32>,
    /// Unit direction from A towards B: move B along it (or A against it) to separate.
    pub normal: Option<Vec2>,
    /// Witness points on A and B: closest points when apart, deepest points when overlapping.
    pub points: Option<(Vec2, Vec2)>,
}

impl Contact {
    pub fn boolean(intersect: bool) -> Self {
        Self {
            intersect,
            ..Self::default()
        }
    }

    /// Contact from a signed distance; overlaps at zero distance count as touching.
    pub fn signed(distance: f32, normal: Option<Vec2>, points: Option<(Vec2, Vec2)>) -> Self {
        Self {
            intersect: distance <= 0.0,
            distance: Some(distance),
            normal,
            points,
        }
    }

    /// The same contact seen from B.
    pub fn flipped(self) -> Self {
        Self {
            normal: self.normal.map(|n| -n),
            points: self.points.map(|(a, b)| (b, a)),
            ..self
        }
    }
}

/// Colors, mapped to the drafting palette on the JS side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ink = 0,
    Soft = 1,
    Faint = 2,
    Cobalt = 3,
    Signal = 4,
    Verdigris = 5,
    Amber = 6,
}

/// A construction mark drawn on top of the shapes to explain what a method did.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    Segment {
        a: Vec2,
        b: Vec2,
        tone: Tone,
        dashed: bool,
    },
    /// Dimension line with arrowheads, offset sideways by `offset` px.
    Dimension {
        a: Vec2,
        b: Vec2,
        offset: f32,
        tone: Tone,
        text: String,
    },
    Point {
        p: Vec2,
        tone: Tone,
    },
    Circle {
        c: Vec2,
        r: f32,
        tone: Tone,
        dashed: bool,
    },
    Polygon {
        points: Vec<Vec2>,
        tone: Tone,
        dashed: bool,
        fill: bool,
    },
    Label {
        p: Vec2,
        tone: Tone,
        text: String,
    },
    /// The interval `[t0, t1]` along the line `origin + dir * t`, drawn with end ticks.
    Interval {
        origin: Vec2,
        dir: Vec2,
        t0: f32,
        t1: f32,
        tone: Tone,
    },
    Arrow {
        a: Vec2,
        b: Vec2,
        tone: Tone,
    },
}

impl Note {
    pub fn segment(a: Vec2, b: Vec2, tone: Tone) -> Self {
        Note::Segment {
            a,
            b,
            tone,
            dashed: false,
        }
    }

    pub fn dashed(a: Vec2, b: Vec2, tone: Tone) -> Self {
        Note::Segment {
            a,
            b,
            tone,
            dashed: true,
        }
    }

    pub fn dimension(a: Vec2, b: Vec2, offset: f32, tone: Tone, text: impl Into<String>) -> Self {
        Note::Dimension {
            a,
            b,
            offset,
            tone,
            text: text.into(),
        }
    }

    pub fn label(p: Vec2, tone: Tone, text: impl Into<String>) -> Self {
        Note::Label {
            p,
            tone,
            text: text.into(),
        }
    }

    /// Flat encoding for the JS renderer: per note
    /// `[kind, tone, flags, label, count, ...count floats]`, where `label`
    /// indexes into `labels` (or is -1) and flags are 1 = dashed, 2 = filled.
    pub fn encode(&self, out: &mut Vec<f32>, labels: &mut Vec<String>) {
        let mut text = |s: &str| {
            labels.push(s.to_owned());
            (labels.len() - 1) as f32
        };
        let (kind, tone, flags, label, data): (u8, Tone, u8, f32, Vec<f32>) = match self {
            Note::Segment { a, b, tone, dashed } => {
                (0, *tone, *dashed as u8, -1.0, vec![a.x, a.y, b.x, b.y])
            }
            Note::Dimension {
                a,
                b,
                offset,
                tone,
                text: s,
            } => (1, *tone, 0, text(s), vec![a.x, a.y, b.x, b.y, *offset]),
            Note::Point { p, tone } => (2, *tone, 0, -1.0, vec![p.x, p.y]),
            Note::Circle { c, r, tone, dashed } => {
                (3, *tone, *dashed as u8, -1.0, vec![c.x, c.y, *r])
            }
            Note::Polygon {
                points,
                tone,
                dashed,
                fill,
            } => (
                4,
                *tone,
                *dashed as u8 | (*fill as u8) << 1,
                -1.0,
                points.iter().flat_map(|p| [p.x, p.y]).collect(),
            ),
            Note::Label { p, tone, text: s } => (5, *tone, 0, text(s), vec![p.x, p.y]),
            Note::Interval {
                origin,
                dir,
                t0,
                t1,
                tone,
            } => (
                6,
                *tone,
                0,
                -1.0,
                vec![origin.x, origin.y, dir.x, dir.y, *t0, *t1],
            ),
            Note::Arrow { a, b, tone } => (7, *tone, 0, -1.0, vec![a.x, a.y, b.x, b.y]),
        };
        out.extend_from_slice(&[
            kind as f32,
            tone as u8 as f32,
            flags as f32,
            label,
            data.len() as f32,
        ]);
        out.extend_from_slice(&data);
    }
}

/// Short number formatting for labels.
pub fn fmt(v: f32) -> String {
    format!("{v:.1}")
}
