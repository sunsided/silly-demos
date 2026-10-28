//! 2×2 covariance matrices: the shape of an elliptical cluster.

use crate::Point;

/// A symmetric 2×2 covariance matrix [[xx, xy], [xy, yy]].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cov {
    /// Variance along x.
    pub xx: f32,
    /// Covariance of x and y.
    pub xy: f32,
    /// Variance along y.
    pub yy: f32,
}

impl Cov {
    /// σ² times the identity: a round spread of standard deviation σ.
    pub fn round(sigma: f32) -> Cov {
        Cov {
            xx: sigma * sigma,
            xy: 0.0,
            yy: sigma * sigma,
        }
    }

    /// The outer product d dᵀ.
    pub fn outer(d: Point) -> Cov {
        Cov {
            xx: d[0] * d[0],
            xy: d[0] * d[1],
            yy: d[1] * d[1],
        }
    }

    /// `self + s · other`.
    pub fn add(self, other: Cov, s: f32) -> Cov {
        Cov {
            xx: self.xx + s * other.xx,
            xy: self.xy + s * other.xy,
            yy: self.yy + s * other.yy,
        }
    }

    /// `s · self`.
    pub fn scale(self, s: f32) -> Cov {
        Cov {
            xx: s * self.xx,
            xy: s * self.xy,
            yy: s * self.yy,
        }
    }

    /// The determinant: the squared area of the one-σ ellipse, over π².
    pub fn det(self) -> f32 {
        self.xx * self.yy - self.xy * self.xy
    }

    /// The squared Mahalanobis length of `d`, dᵀ Σ⁻¹ d; infinite for a
    /// matrix that isn't positive definite.
    pub fn mahalanobis2(self, d: Point) -> f32 {
        let det = self.det();
        if det <= 0.0 {
            return f32::INFINITY;
        }
        (self.yy * d[0] * d[0] - 2.0 * self.xy * d[0] * d[1] + self.xx * d[1] * d[1]) / det
    }

    /// The eigenvalues, larger first, and the angle of the larger one's
    /// axis from +x in radians: the squared semi-axes of the one-σ ellipse
    /// and its orientation.
    pub fn axes(self) -> (f32, f32, f32) {
        let mean = (self.xx + self.yy) / 2.0;
        let half_gap = ((self.xx - self.yy) / 2.0).hypot(self.xy);
        let angle = 0.5 * (2.0 * self.xy).atan2(self.xx - self.yy);
        (mean + half_gap, (mean - half_gap).max(0.0), angle)
    }

    /// The larger eigenvalue.
    pub fn largest(self) -> f32 {
        self.axes().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rotated_ellipse_has_its_axes_back() {
        // Variances 4 and 1 along axes at 30°.
        let a = 30f32.to_radians();
        let (c, s) = (a.cos(), a.sin());
        let cov = Cov {
            xx: 4.0 * c * c + s * s,
            xy: (4.0 - 1.0) * c * s,
            yy: 4.0 * s * s + c * c,
        };
        let (big, small, angle) = cov.axes();
        assert!((big - 4.0).abs() < 1e-5 && (small - 1.0).abs() < 1e-5);
        assert!((angle - a).abs() < 1e-5);
        assert!((cov.det() - 4.0).abs() < 1e-5);
        // One standard deviation out along either axis is distance 1.
        assert!((cov.mahalanobis2([2.0 * c, 2.0 * s]) - 1.0).abs() < 1e-5);
        assert!((cov.mahalanobis2([-s, c]) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_flat_matrix_puts_everything_infinitely_far() {
        let flat = Cov {
            xx: 1.0,
            xy: 1.0,
            yy: 1.0,
        };
        assert!(flat.mahalanobis2([1.0, 0.0]).is_infinite());
    }
}
