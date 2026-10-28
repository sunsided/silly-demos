//! The cities of a tour and how a tour between them is measured.

/// Cities a set may have; the baselines keep an n × n distance matrix.
pub const MAX_CITIES: usize = 1000;

/// How the distance between two cities is measured. The solvers learn from
/// planar positions either way; the metric only scores their tours.
#[derive(Clone, Debug, PartialEq)]
pub enum Metric {
    /// Straight-line distance.
    Euclid,
    /// TSPLIB's EUC_2D: the straight-line distance rounded to the nearest
    /// integer, the metric its published optima are measured in.
    Tsplib,
    /// Great-circle distance in km between (longitude, latitude) pairs in
    /// degrees, one per city.
    Geo(Vec<[f64; 2]>),
}

/// The Earth's mean radius, km.
const EARTH_RADIUS: f64 = 6371.0;

#[derive(Clone, Debug)]
pub struct Cities {
    xy: Vec<[f32; 2]>,
    unit: Vec<[f32; 2]>,
    origin: [f32; 2],
    scale: f32,
    metric: Metric,
}

impl Cities {
    /// `xy` holds x, y per city. `None` for fewer than 3 or more than
    /// [`MAX_CITIES`] cities, an odd length, a non-finite value, or geo
    /// coordinates that don't match the cities one to one.
    pub fn new(xy: &[f32], metric: Metric) -> Option<Cities> {
        if !xy.len().is_multiple_of(2) || !xy.iter().all(|v| v.is_finite()) {
            return None;
        }
        let xy: Vec<[f32; 2]> = xy.as_chunks::<2>().0.to_vec();
        if !(3..=MAX_CITIES).contains(&xy.len()) {
            return None;
        }
        if let Metric::Geo(lonlat) = &metric
            && (lonlat.len() != xy.len() || !lonlat.iter().flatten().all(|v| v.is_finite()))
        {
            return None;
        }
        // The solvers work in the unit square, keeping the aspect ratio, so
        // their parameters mean the same at any scale.
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for p in &xy {
            for k in 0..2 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let extent = (max[0] - min[0]).max(max[1] - min[1]);
        let scale = if extent > 0.0 { extent } else { 1.0 };
        let unit = xy
            .iter()
            .map(|p| [(p[0] - min[0]) / scale, (p[1] - min[1]) / scale])
            .collect();
        Some(Cities {
            xy,
            unit,
            origin: min,
            scale,
            metric,
        })
    }

    pub fn len(&self) -> usize {
        self.xy.len()
    }

    pub fn is_empty(&self) -> bool {
        self.xy.is_empty()
    }

    /// Positions as given.
    pub fn xy(&self) -> &[[f32; 2]] {
        &self.xy
    }

    /// Positions scaled into the unit square, what the solvers learn from.
    pub fn unit(&self) -> &[[f32; 2]] {
        &self.unit
    }

    /// A unit-square point back in the coordinates the cities were given in.
    pub fn to_input(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + p[0] * self.scale,
            self.origin[1] + p[1] * self.scale,
        ]
    }

    pub fn metric(&self) -> &Metric {
        &self.metric
    }

    pub fn dist(&self, i: usize, j: usize) -> f64 {
        let (a, b) = (self.xy[i], self.xy[j]);
        let euclid = || (a[0] as f64 - b[0] as f64).hypot(a[1] as f64 - b[1] as f64);
        match &self.metric {
            Metric::Euclid => euclid(),
            Metric::Tsplib => (euclid() + 0.5).floor(),
            Metric::Geo(lonlat) => haversine(lonlat[i], lonlat[j]),
        }
    }

    /// The length of the closed tour visiting `tour` in order.
    pub fn tour_length(&self, tour: &[u32]) -> f64 {
        let n = tour.len();
        (0..n)
            .map(|k| self.dist(tour[k] as usize, tour[(k + 1) % n] as usize))
            .sum()
    }

    /// Every pairwise distance, for the baselines.
    pub fn matrix(&self) -> Matrix {
        let n = self.len();
        let mut d = vec![0.0; n * n];
        for i in 0..n {
            for j in i + 1..n {
                let v = self.dist(i, j);
                d[i * n + j] = v;
                d[j * n + i] = v;
            }
        }
        Matrix { n, d }
    }
}

/// Great-circle distance in km between two (longitude, latitude) points.
pub fn haversine(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (lon1, lat1) = (a[0].to_radians(), a[1].to_radians());
    let (lon2, lat2) = (b[0].to_radians(), b[1].to_radians());
    let h = ((lat2 - lat1) / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * ((lon2 - lon1) / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS * h.sqrt().min(1.0).asin()
}

/// Pairwise distances, row-major.
#[derive(Clone, Debug)]
pub struct Matrix {
    n: usize,
    d: Vec<f64>,
}

impl Matrix {
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn get(&self, i: u32, j: u32) -> f64 {
        self.d[i as usize * self.n + j as usize]
    }

    pub fn tour_length(&self, tour: &[u32]) -> f64 {
        let n = tour.len();
        (0..n).map(|k| self.get(tour[k], tour[(k + 1) % n])).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_what_it_cant_measure() {
        assert!(Cities::new(&[0.0, 0.0, 1.0, 1.0], Metric::Euclid).is_none());
        assert!(Cities::new(&[0.0, 0.0, 1.0, 1.0, 2.0], Metric::Euclid).is_none());
        assert!(Cities::new(&[0.0, 0.0, 1.0, f32::NAN, 2.0, 2.0], Metric::Euclid).is_none());
        assert!(Cities::new(&vec![0.0; 2 * (MAX_CITIES + 1)], Metric::Euclid).is_none());
        let three = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        assert!(Cities::new(&three, Metric::Geo(vec![[0.0, 0.0]; 2])).is_none());
        assert!(Cities::new(&three, Metric::Geo(vec![[0.0, 0.0]; 3])).is_some());
        assert!(Cities::new(&three, Metric::Euclid).is_some());
    }

    #[test]
    fn tsplib_rounds_to_the_nearest_integer() {
        // 3-4-5 is exact; 1-1 is 1.414 → 1; 1-2 is 2.236 → 2; 3-3 is 4.243 → 4; 0.5-0 is 0.5 → 1.
        let c = Cities::new(
            &[0.0, 0.0, 3.0, 4.0, 1.0, 1.0, 1.0, 2.0, 3.0, 3.0, 0.5, 0.0],
            Metric::Tsplib,
        )
        .unwrap();
        assert_eq!(c.dist(0, 1), 5.0);
        assert_eq!(c.dist(0, 2), 1.0);
        assert_eq!(c.dist(0, 3), 2.0);
        assert_eq!(c.dist(0, 4), 4.0);
        assert_eq!(c.dist(0, 5), 1.0);
    }

    #[test]
    fn berlin_to_paris_is_about_878_km() {
        let d = haversine([13.405, 52.52], [2.3522, 48.8566]);
        assert!((d - 878.0).abs() < 5.0, "{d}");
    }

    #[test]
    fn the_unit_square_keeps_the_aspect_ratio() {
        let c = Cities::new(&[10.0, 10.0, 210.0, 10.0, 110.0, 60.0], Metric::Euclid).unwrap();
        assert_eq!(c.unit(), &[[0.0, 0.0], [1.0, 0.0], [0.5, 0.25]]);
        assert_eq!(c.to_input([0.5, 0.25]), [110.0, 60.0]);
    }

    #[test]
    fn the_matrix_agrees_with_the_metric() {
        let c = Cities::new(&[0.0, 0.0, 3.0, 4.0, 6.0, 0.0], Metric::Euclid).unwrap();
        let m = c.matrix();
        assert_eq!(m.get(0, 1), 5.0);
        assert_eq!(m.get(2, 0), 6.0);
        assert_eq!(m.tour_length(&[0, 1, 2]), 16.0);
        assert_eq!(c.tour_length(&[0, 1, 2]), 16.0);
    }
}
