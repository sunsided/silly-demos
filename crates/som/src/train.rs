//! Training a map: Kohonen's online rule, one sample at a time, and the
//! batch map, which moves every node at once per pass over the data.

use crate::codebook::Codebook;
use crate::lattice::Lattice;
use crate::schedule::{Schedule, kernel};

/// What one update did, for drawing it.
#[derive(Clone, Debug)]
pub struct StepTrace {
    /// The sample shown; `None` for a batch epoch.
    pub sample: Option<Vec<f32>>,
    /// Its best-matching unit; `None` for a batch epoch.
    pub bmu: Option<usize>,
    /// The kernel value h of every node; all 1 for a batch epoch.
    pub kernel: Vec<f32>,
    /// The weights before the update.
    pub previous: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct Som {
    pub lattice: Lattice,
    pub codebook: Codebook,
    pub schedule: Schedule,
    /// Samples presented so far.
    pub t: u64,
}

impl Som {
    pub fn finished(&self) -> bool {
        self.schedule.finished(self.t)
    }

    /// Kohonen's rule: every node moves towards `x` by α · h, where h is
    /// the kernel at its lattice distance from the best-matching unit.
    /// Does nothing once finished. Returns the trace when asked for one.
    pub fn step_online(&mut self, x: &[f32], trace: bool) -> Option<StepTrace> {
        if self.finished() {
            return None;
        }
        let (alpha, sigma) = self.schedule.at(self.t);
        let bmu = self.codebook.bmu(x);
        let previous = trace.then(|| self.codebook.weights.clone());
        let dim = self.codebook.dim;
        let mut kernels = Vec::with_capacity(if trace { self.lattice.len() } else { 0 });
        for (i, w) in self.codebook.weights.chunks_exact_mut(dim).enumerate() {
            let h = kernel(self.lattice.dist2(bmu, i), sigma);
            w.iter_mut()
                .zip(x)
                .for_each(|(w, x)| *w += alpha * h * (x - *w));
            if trace {
                kernels.push(h);
            }
        }
        self.t += 1;
        previous.map(|previous| StepTrace {
            sample: Some(x.to_vec()),
            bmu: Some(bmu),
            kernel: kernels,
            previous,
        })
    }

    /// The batch map: assign every sample to its best-matching unit, then
    /// set every node to the kernel-weighted mean of all samples, weighted
    /// by how close (on the lattice) the node that won them is. There is no
    /// learning rate. A node no sample reaches keeps its weight. Counts as
    /// presenting every sample once.
    pub fn epoch_batch(&mut self, samples: &[f32], trace: bool) -> Option<StepTrace> {
        let dim = self.codebook.dim;
        let count = samples.len() / dim;
        if self.finished() || count == 0 {
            return None;
        }
        let (_, sigma) = self.schedule.at(self.t);
        let n = self.codebook.len();
        let mut sums = vec![0.0f32; n * dim];
        let mut hits = vec![0.0f32; n];
        for x in samples.chunks_exact(dim) {
            let j = self.codebook.bmu(x);
            hits[j] += 1.0;
            sums[j * dim..(j + 1) * dim]
                .iter_mut()
                .zip(x)
                .for_each(|(s, x)| *s += x);
        }
        let previous = trace.then(|| self.codebook.weights.clone());
        let winners: Vec<usize> = (0..n).filter(|&j| hits[j] > 0.0).collect();
        let mut num = vec![0.0f32; dim];
        for i in 0..n {
            num.iter_mut().for_each(|v| *v = 0.0);
            let mut den = 0.0;
            for &j in &winners {
                let h = kernel(self.lattice.dist2(i, j), sigma);
                den += h * hits[j];
                num.iter_mut()
                    .zip(&sums[j * dim..(j + 1) * dim])
                    .for_each(|(a, s)| *a += h * s);
            }
            if den > 1e-12 {
                let w = &mut self.codebook.weights[i * dim..(i + 1) * dim];
                w.iter_mut().zip(&num).for_each(|(w, a)| *w = a / den);
            }
        }
        self.t += count as u64;
        previous.map(|previous| StepTrace {
            sample: None,
            bmu: None,
            kernel: vec![1.0; n],
            previous,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::density::Density;
    use crate::init::{Init, initialise};
    use crate::lattice::Kind;
    use crate::metrics::{quantization_error, topographic_error};
    use crate::rng::Rng;
    use crate::schedule::Decay;

    /// The seed whose bow-tie run twists (see `bow_tie_keeps_a_twist`).
    const BOW_TIE_SEED: u64 = 14;

    struct Run {
        som: Som,
        density: Density,
        rng: Rng,
        eval: Vec<f32>,
    }

    fn draw(density: &mut Density, rng: &mut Rng, n: usize) -> Vec<f32> {
        (0..n).flat_map(|_| density.sample(rng).unwrap()).collect()
    }

    fn run(
        kind: Kind,
        w: usize,
        h: usize,
        init: Init,
        sigma: (f32, f32),
        steps: u64,
        seed: u64,
    ) -> Run {
        let mut density = Density::preset("uniform").unwrap();
        let mut rng = Rng::new(seed);
        let eval = draw(&mut density, &mut rng, 1000);
        let lattice = Lattice::new(kind, w, h).unwrap();
        let codebook = initialise(init, &lattice, &eval, 2, &mut rng);
        let schedule = Schedule {
            steps,
            alpha: (0.5, 0.01),
            sigma,
            decay: Decay::Exponential,
            continuous: false,
        };
        Run {
            som: Som {
                lattice,
                codebook,
                schedule,
                t: 0,
            },
            density,
            rng,
            eval,
        }
    }

    fn classic(seed: u64) -> Run {
        run(Kind::Grid, 12, 12, Init::Random, (6.0, 0.5), 20_000, seed)
    }

    fn bow_tie(seed: u64) -> Run {
        run(Kind::Grid, 12, 12, Init::Cluster, (1.0, 0.3), 10_000, seed)
    }

    impl Run {
        fn online(&mut self, n: usize) {
            for _ in 0..n {
                let x = self.density.sample(&mut self.rng).unwrap();
                self.som.step_online(&x, false);
            }
        }

        fn te(&self) -> f32 {
            topographic_error(&self.som.codebook, &self.som.lattice, &self.eval)
        }

        fn qe(&self) -> f32 {
            quantization_error(&self.som.codebook, &self.eval)
        }
    }

    #[test]
    fn classic_unfold_orders_the_grid() {
        let mut r = classic(1);
        r.online(20_000);
        assert!(r.som.finished());
        assert!(r.te() < 0.05, "TE {}", r.te());
        assert!(r.qe() < 0.05, "QE {}", r.qe());
    }

    #[test]
    fn bow_tie_keeps_a_twist() {
        let mut c = classic(BOW_TIE_SEED);
        c.online(20_000);
        let mut b = bow_tie(BOW_TIE_SEED);
        b.online(10_000);
        assert!(b.som.finished());
        assert!(
            b.te() > 0.1 && b.te() > 3.0 * c.te(),
            "bow-tie {} vs classic {}",
            b.te(),
            c.te()
        );
    }

    impl Run {
        /// Ordered and spread over the data. TE alone is no test: early on,
        /// a wide σ pulls the map into an ordered clump with TE near 0.
        fn converged(&self) -> bool {
            self.te() < 0.05 && self.qe() < 0.05
        }
    }

    #[test]
    fn batch_converges_within_its_schedule() {
        let mut r = classic(2);
        let mut epochs = 0;
        while !r.som.finished() {
            let samples = draw(&mut r.density, &mut r.rng, 2000);
            r.som.epoch_batch(&samples, false);
            epochs += 1;
        }
        assert_eq!(epochs, 10);
        assert!(r.converged(), "TE {} QE {}", r.te(), r.qe());
    }

    #[test]
    fn batch_ignores_sample_order() {
        let (mut a, mut b) = (classic(6), classic(6));
        let samples = draw(&mut a.density, &mut a.rng, 2000);
        let reversed: Vec<f32> = samples.chunks(2).rev().flatten().copied().collect();
        a.som.epoch_batch(&samples, false);
        b.som.epoch_batch(&reversed, false);
        for (x, y) in a.som.codebook.weights.iter().zip(&b.som.codebook.weights) {
            assert!((x - y).abs() < 1e-4, "{x} vs {y}");
        }
    }

    #[test]
    fn online_stops_when_finished() {
        let mut r = classic(3);
        r.online(20_000);
        let before = r.som.codebook.clone();
        assert!(r.som.step_online(&[0.0, 0.0], true).is_none());
        assert!(r.som.epoch_batch(&[0.0, 0.0], true).is_none());
        assert_eq!((r.som.codebook.clone(), r.som.t), (before, 20_000));
    }

    #[test]
    fn online_trace_shows_the_update() {
        let mut r = classic(4);
        let before = r.som.codebook.weights.clone();
        let trace = r.som.step_online(&[0.5, 0.5], true).unwrap();
        assert_eq!(trace.sample.as_deref(), Some(&[0.5, 0.5][..]));
        let bmu = trace.bmu.unwrap();
        assert_eq!(trace.kernel[bmu], 1.0);
        assert_eq!(trace.kernel.len(), 144);
        assert_eq!(trace.previous, before);
        assert_eq!(r.som.t, 1);
        // The BMU moved halfway (α = 0.5, h = 1) to the sample.
        let (old, new) = (&before[bmu * 2..bmu * 2 + 2], r.som.codebook.node(bmu));
        assert!((new[0] - (old[0] + 0.5) / 2.0).abs() < 1e-6);
    }

    #[test]
    fn batch_trace_has_previous_weights_only() {
        let mut r = classic(4);
        let before = r.som.codebook.weights.clone();
        let samples = draw(&mut r.density, &mut r.rng, 2000);
        let trace = r.som.epoch_batch(&samples, true).unwrap();
        assert!(trace.sample.is_none() && trace.bmu.is_none());
        assert_eq!(trace.previous, before);
        assert_eq!(r.som.t, 2000);
    }

    #[test]
    fn odd_but_valid_schedules_stay_finite() {
        let mut r = classic(5);
        r.som.schedule = Schedule {
            steps: 1,
            alpha: (1.0, 1.0),
            sigma: (0.5, 6.0),
            decay: Decay::Exponential,
            continuous: true,
        };
        assert!(r.som.schedule.is_valid());
        r.online(50);
        for _ in 0..2 {
            let samples = draw(&mut r.density, &mut r.rng, 2000);
            r.som.epoch_batch(&samples, false);
        }
        assert!(r.som.codebook.weights.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn batch_keeps_nodes_without_data() {
        let lattice = Lattice::new(Kind::Chain, 3, 1).unwrap();
        let codebook = Codebook {
            dim: 2,
            weights: vec![0.0, 0.0, 0.1, 0.0, 9.0, 9.0],
        };
        let schedule = Schedule {
            steps: 100,
            alpha: (0.5, 0.5),
            sigma: (1e-3, 1e-3),
            decay: Decay::Linear,
            continuous: false,
        };
        let mut som = Som {
            lattice,
            codebook,
            schedule,
            t: 0,
        };
        som.epoch_batch(&[0.0, 0.2, 0.2, 0.0], false);
        assert_eq!(som.codebook.node(2), &[9.0, 9.0]);
        // The two that won a sample each sit on it.
        assert_eq!(som.codebook.node(0), &[0.0, 0.2]);
        assert_eq!(som.codebook.node(1), &[0.2, 0.0]);
    }
}
