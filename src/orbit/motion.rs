// Animation channels: an optional delay and an eased transition, then a hold
// or a sine wave. They are turned into DirectComposition animations.

use std::ops::{Add, Mul, Sub};

use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

pub fn now() -> f64 {
    let mut counter = 0i64;
    let mut frequency = 0i64;
    unsafe {
        let _ = QueryPerformanceCounter(&mut counter);
        let _ = QueryPerformanceFrequency(&mut frequency);
    }
    counter as f64 / frequency as f64
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ellipse {
    pub center: Vec2,
    pub rx: f64,
    pub ry: f64,
    pub dir: f64,
}

impl Ellipse {
    pub fn pos(&self, angle: f64) -> Vec2 {
        Vec2::new(
            self.center.x + self.rx * angle.cos(),
            self.center.y + self.dir * self.ry * angle.sin(),
        )
    }

    pub fn depth(&self, angle: f64) -> f64 {
        self.dir * angle.sin()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ease {
    pub dur: f64,
    pub c: [f64; 4],
}

impl Ease {
    pub fn hermite(v0: f64, s0: f64, v1: f64, s1: f64, dur: f64) -> Self {
        let d = v1 - v0;
        let c2 = (3.0 * d - (2.0 * s0 + s1) * dur) / (dur * dur);
        let c3 = ((s0 + s1) * dur - 2.0 * d) / (dur * dur * dur);
        Self {
            dur,
            c: [v0, s0, c2, c3],
        }
    }

    pub fn value(&self, t: f64) -> f64 {
        let [c0, c1, c2, c3] = self.c;
        c0 + c1 * t + c2 * t * t + c3 * t * t * t
    }

    pub fn slope(&self, t: f64) -> f64 {
        let [_, c1, c2, c3] = self.c;
        c1 + 2.0 * c2 * t + 3.0 * c3 * t * t
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Wave {
    pub bias: f64,
    pub amp: f64,
    pub omega: f64,
    pub phase: f64,
}

impl Wave {
    pub fn value(&self, t: f64) -> f64 {
        self.bias + self.amp * (self.omega * t + self.phase).sin()
    }

    pub fn slope(&self, t: f64) -> f64 {
        self.amp * self.omega * (self.omega * t + self.phase).cos()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Tail {
    Hold(f64),
    Wave(Wave),
}

impl Tail {
    fn sample(&self, t: f64) -> (f64, f64) {
        match self {
            Tail::Hold(value) => (*value, 0.0),
            Tail::Wave(wave) => (wave.value(t), wave.slope(t)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub value: f64,
    pub slope: f64,
    pub tail: Tail,
}

impl Target {
    pub fn hold(value: f64) -> Self {
        Self {
            value,
            slope: 0.0,
            tail: Tail::Hold(value),
        }
    }

    pub fn wave(wave: Wave) -> Self {
        Self {
            value: wave.value(0.0),
            slope: wave.slope(0.0),
            tail: Tail::Wave(wave),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Channel {
    pub start: f64,
    pub delay: f64,
    pub lead: Option<Ease>,
    pub tail: Tail,
}

impl Channel {
    pub fn fixed(value: f64) -> Self {
        Self {
            start: 0.0,
            delay: 0.0,
            lead: None,
            tail: Tail::Hold(value),
        }
    }

    pub fn fixed_value(&self) -> Option<f64> {
        match (self.lead, self.tail) {
            (None, Tail::Hold(value)) if self.delay == 0.0 => Some(value),
            _ => None,
        }
    }

    pub fn sample(&self, t: f64) -> (f64, f64) {
        let mut local = t - self.start - self.delay;
        if let Some(lead) = &self.lead {
            if local < 0.0 {
                return (lead.c[0], 0.0);
            }
            if local < lead.dur {
                return (lead.value(local), lead.slope(local));
            }
            local -= lead.dur;
        }
        self.tail.sample(local.max(0.0))
    }

    pub fn rebase(&self, now: f64) -> Self {
        let mut local = now - self.start - self.delay;
        if local < 0.0 {
            return Self {
                start: now,
                delay: -local,
                ..*self
            };
        }
        if let Some(lead) = self.lead {
            if local < lead.dur {
                let [_, _, c2, c3] = lead.c;
                let shifted = Ease {
                    dur: lead.dur - local,
                    c: [
                        lead.value(local),
                        lead.slope(local),
                        c2 + 3.0 * c3 * local,
                        c3,
                    ],
                };
                return Self {
                    start: now,
                    delay: 0.0,
                    lead: Some(shifted),
                    tail: self.tail,
                };
            }
            local -= lead.dur;
        }
        let tail = match self.tail {
            Tail::Wave(wave) => Tail::Wave(Wave {
                phase: (wave.phase + wave.omega * local).rem_euclid(std::f64::consts::TAU),
                ..wave
            }),
            hold => hold,
        };
        Self {
            start: now,
            delay: 0.0,
            lead: None,
            tail,
        }
    }

    pub fn to(&self, t: f64, delay: f64, dur: f64, target: Target, kick: bool) -> Self {
        let (value, slope) = self.sample(t);
        let settled = (value - target.value).abs() < 1e-3 && slope.abs() < 1e-6;
        if settled && delay == 0.0 && matches!(target.tail, Tail::Hold(_)) {
            return Self::fixed(target.value);
        }
        let slope = if kick {
            slope + 2.0 * (target.value - value) / dur
        } else {
            slope
        };
        Self {
            start: t,
            delay,
            lead: Some(Ease::hermite(value, slope, target.value, target.slope, dur)),
            tail: target.tail,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub speed: f64,
    pub running: bool,
    base_angle: f64,
    base_time: f64,
}

impl Clock {
    pub fn new(speed: f64, running: bool, t: f64) -> Self {
        Self {
            speed,
            running,
            base_angle: 0.0,
            base_time: t,
        }
    }

    pub fn angle(&self, t: f64) -> f64 {
        if self.running {
            self.base_angle + self.speed * (t - self.base_time)
        } else {
            self.base_angle
        }
    }

    pub fn pause(&mut self, t: f64, coast: f64) {
        self.base_angle = self.angle(t) + coast;
        self.running = false;
    }

    pub fn resume(&mut self, t: f64, delay: f64) {
        self.base_time = t + delay;
        self.running = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn hermite_hits_both_ends() {
        let ease = Ease::hermite(1.0, 3.0, 10.0, 0.5, 0.4);
        assert!(close(ease.value(0.0), 1.0));
        assert!(close(ease.slope(0.0), 3.0));
        assert!(close(ease.value(0.4), 10.0));
        assert!(close(ease.slope(0.4), 0.5));
    }

    #[test]
    fn channel_waits_then_moves_then_follows_wave() {
        let wave = Wave {
            bias: 5.0,
            amp: 2.0,
            omega: 1.0,
            phase: 0.3,
        };
        let channel = Channel::fixed(0.0).to(10.0, 0.5, 1.0, Target::wave(wave), false);
        assert!(close(channel.sample(10.2).0, 0.0));
        assert!(close(channel.sample(11.5).0, wave.value(0.0)));
        assert!(close(channel.sample(13.5).0, wave.value(2.0)));
    }

    #[test]
    fn rebase_keeps_the_same_motion() {
        let wave = Wave {
            bias: 5.0,
            amp: 2.0,
            omega: 1.3,
            phase: 0.3,
        };
        let channel = Channel::fixed(0.0).to(10.0, 0.5, 1.0, Target::wave(wave), false);
        for now in [10.2, 10.9, 11.2, 14.0] {
            let rebased = channel.rebase(now);
            for dt in [0.0, 0.05, 0.4, 0.8, 3.0] {
                let (a, b) = (channel.sample(now + dt), rebased.sample(now + dt));
                assert!(
                    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
                    "now {now} dt {dt}"
                );
            }
        }
    }

    #[test]
    fn settled_channel_needs_no_animation() {
        let channel = Channel::fixed(4.0).to(1.0, 0.0, 0.3, Target::hold(4.0), false);
        assert_eq!(channel.fixed_value(), Some(4.0));
    }

    #[test]
    fn clock_pause_and_resume() {
        let mut clock = Clock::new(2.0, true, 10.0);
        assert_eq!(clock.angle(11.0), 2.0);
        clock.pause(11.0, 0.5);
        assert_eq!(clock.angle(20.0), 2.5);
        clock.resume(20.0, 1.0);
        assert_eq!(clock.angle(21.0), 2.5);
        assert_eq!(clock.angle(22.0), 4.5);
    }
}
