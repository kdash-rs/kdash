use rand::distributions::{Distribution, Uniform};
use rand::rngs::ThreadRng;
use tui::widgets::ListState;

#[derive(Clone)]
pub struct RandomSignal {
  distribution: Uniform<u64>,
  rng: ThreadRng,
}

impl RandomSignal {
  pub fn new(lower: u64, upper: u64) -> RandomSignal {
    RandomSignal {
      distribution: Uniform::new(lower, upper),
      rng: rand::thread_rng(),
    }
  }
}

impl Iterator for RandomSignal {
  type Item = u64;
  fn next(&mut self) -> Option<u64> {
    Some(self.distribution.sample(&mut self.rng))
  }
}

#[derive(Clone)]
pub struct SinSignal {
  x: f64,
  interval: f64,
  period: f64,
  scale: f64,
}

impl SinSignal {
  pub fn new(interval: f64, period: f64, scale: f64) -> SinSignal {
    SinSignal {
      x: 0.0,
      interval,
      period,
      scale,
    }
  }
}

impl Iterator for SinSignal {
  type Item = (f64, f64);
  fn next(&mut self) -> Option<Self::Item> {
    let point = (self.x, (self.x * 1.0 / self.period).sin() * self.scale);
    self.x += self.interval;
    Some(point)
  }
}
