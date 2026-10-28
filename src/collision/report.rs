//! Narrowphase results: the [`Contact`] a method reports and the construction
//! [`Note`]s it leaves behind for drawing.

use crate::vec2::Vec2;

/// Outcome of a narrowphase test. Methods only fill in what they compute:
/// a boolean test leaves `distance`, `normal` and `points` empty.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// Whether the shapes touch or overlap.
    pub intersect: bool,
    /// Signed distance: positive gap when apart, negative penetration depth when overlapping.
    pub distance: Option<f32>,
    /// Unit direction from A towards B: move B along it (or A against it) to separate.
    pub normal: Option<Vec2>,
    /// Witness points on A and B: closest points when apart, deepest points when overlapping.
    pub points: Option<(Vec2, Vec2)>,
}

impl Contact {
    /// A yes/no result with no distance, normal or witness points.
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
    /// Main ink for key points and shape A's marks.
    Ink = 0,
    /// Muted ink for helper lines and captions.
    Soft = 1,
    /// Barely-there ink for projector lines and empty cells.
    Faint = 2,
    /// Blue for shape B, gaps and rejected pairs.
    Cobalt = 3,
    /// Alert color for overlaps and hits.
    Signal = 4,
    /// Green for capsule cores and cells holding only one of A or B.
    Verdigris = 5,
    /// Amber for quadtree cells shared by A and B.
    Amber = 6,
}

/// A construction mark drawn on top of the shapes to explain what a method did.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    /// A straight line from `a` to `b`.
    Segment {
        /// Start point.
        a: Vec2,
        /// End point.
        b: Vec2,
        /// Color of the mark.
        tone: Tone,
        /// Whether the line is dashed.
        dashed: bool,
    },
    /// Dimension line with arrowheads, offset sideways by `offset` px.
    Dimension {
        /// Start of the measured span.
        a: Vec2,
        /// End of the measured span.
        b: Vec2,
        /// Sideways offset of the dimension line in px; the sign picks the side.
        offset: f32,
        /// Color of the mark.
        tone: Tone,
        /// Label written along the line.
        text: String,
    },
    /// A dot marking a single point.
    Point {
        /// Where the dot sits.
        p: Vec2,
        /// Color of the mark.
        tone: Tone,
    },
    /// A circle outline.
    Circle {
        /// Center.
        c: Vec2,
        /// Radius in px.
        r: f32,
        /// Color of the mark.
        tone: Tone,
        /// Whether the outline is dashed.
        dashed: bool,
    },
    /// A closed polyline, optionally filled.
    Polygon {
        /// Vertices in drawing order; the last joins back to the first.
        points: Vec<Vec2>,
        /// Color of the mark.
        tone: Tone,
        /// Whether the outline is dashed.
        dashed: bool,
        /// Whether the interior is shaded.
        fill: bool,
    },
    /// Free text anchored at a point.
    Label {
        /// Anchor of the text.
        p: Vec2,
        /// Color of the text.
        tone: Tone,
        /// Text to draw.
        text: String,
    },
    /// The interval `[t0, t1]` along the line `origin + dir * t`, drawn with end ticks.
    Interval {
        /// Point on the line where `t` is zero.
        origin: Vec2,
        /// Direction of the line; `t` is measured in multiples of it.
        dir: Vec2,
        /// Start parameter.
        t0: f32,
        /// End parameter.
        t1: f32,
        /// Color of the mark.
        tone: Tone,
    },
    /// An arrow from `a` pointing at `b`.
    Arrow {
        /// Tail.
        a: Vec2,
        /// Head.
        b: Vec2,
        /// Color of the mark.
        tone: Tone,
    },
}

impl Note {
    /// A solid [`Note::Segment`].
    pub fn segment(a: Vec2, b: Vec2, tone: Tone) -> Self {
        Note::Segment {
            a,
            b,
            tone,
            dashed: false,
        }
    }

    /// A dashed [`Note::Segment`].
    pub fn dashed(a: Vec2, b: Vec2, tone: Tone) -> Self {
        Note::Segment {
            a,
            b,
            tone,
            dashed: true,
        }
    }

    /// A [`Note::Dimension`] taking any string-like label.
    pub fn dimension(a: Vec2, b: Vec2, offset: f32, tone: Tone, text: impl Into<String>) -> Self {
        Note::Dimension {
            a,
            b,
            offset,
            tone,
            text: text.into(),
        }
    }

    /// A [`Note::Label`] taking any string-like text.
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
