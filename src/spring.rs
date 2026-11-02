// Pillify — Spring Physics Engine (Damped Harmonic Oscillator)
// Provides NUSA/iOS-style fluid, interruptible animations with real velocity preservation.

#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    pub const fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
            stiffness,
            damping,
        }
    }

    /// Snappy preset (responsive UI controls, quick hover)
    pub const fn snappy(initial: f32) -> Self {
        Self::new(initial, 360.0, 26.0)
    }

    /// Bouncy preset (Dynamic Island capsule expand/morph)
    pub const fn bouncy(initial: f32) -> Self {
        Self::new(initial, 280.0, 19.5)
    }

    /// Smooth preset (critically damped, zero overshoot)
    pub const fn smooth(initial: f32) -> Self {
        Self::new(initial, 240.0, 28.0)
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    #[allow(dead_code)]
    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    pub fn apply_preset(&mut self, stiffness: f32, damping: f32) {
        self.stiffness = stiffness.clamp(60.0, 900.0);
        self.damping = damping.clamp(8.0, 80.0);
    }

    /// Advances the spring by `dt` seconds using semi-implicit Euler sub-stepping for stability.
    /// Returns `true` if the spring is still in motion.
    pub fn step(&mut self, dt: f32) -> bool {
        let clamped_dt = dt.clamp(0.0005, 0.05);
        // Sub-step in 4ms increments so high stiffness never explodes on frame spikes
        let steps = ((clamped_dt / 0.004).ceil() as usize).max(1);
        let sub_dt = clamped_dt / (steps as f32);

        for _ in 0..steps {
            let displacement = self.value - self.target;
            let spring_force = -self.stiffness * displacement;
            let damping_force = -self.damping * self.velocity;
            let acceleration = spring_force + damping_force;

            self.velocity += acceleration * sub_dt;
            self.value += self.velocity * sub_dt;
        }

        if (self.value - self.target).abs() < 0.08 && self.velocity.abs() < 0.35 {
            self.value = self.target;
            self.velocity = 0.0;
            false
        } else {
            true
        }
    }

    #[allow(dead_code)]
    pub fn is_animating(&self) -> bool {
        (self.value - self.target).abs() >= 0.08 || self.velocity.abs() >= 0.35
    }
}

#[derive(Debug, Clone)]
pub struct IslandAnimator {
    pub width: Spring,
    pub height: Spring,
    pub top_offset: Spring,
    pub corner_radius: Spring,
    pub notch_flare: Spring, // 0.0 = floating rounded pill, 1.0 = MacBook concave top ears
    pub expand_progress: Spring, // 0.0 = compact bar, 1.0 = expanded dashboard
    pub hover_scale: Spring, // subtle magnetic scale on hover
    pub compact_alert_blend: Spring, // 0.0 = normal compact, 1.0 = toast alert compact
    pub page_scroll: Spring, // 0.0 = Media, 1.0 = Shelf, 2.0 = Focus/Timer, 3.0 = Customizer
    pub osd_blend: Spring, // 0.0 = hidden, 1.0 = Volume/System OSD active
}

impl IslandAnimator {
    pub fn new(base_w: f32, base_h: f32, top_margin: f32, radius: f32, is_notch: bool) -> Self {
        Self {
            width: Spring::bouncy(base_w),
            height: Spring::bouncy(base_h),
            top_offset: Spring::bouncy(if is_notch { 0.0 } else { top_margin }),
            corner_radius: Spring::smooth(radius),
            notch_flare: Spring::smooth(if is_notch { 1.0 } else { 0.0 }),
            expand_progress: Spring::smooth(0.0),
            hover_scale: Spring::snappy(1.0),
            compact_alert_blend: Spring::snappy(0.0),
            page_scroll: Spring::snappy(0.0),
            osd_blend: Spring::snappy(0.0),
        }
    }

    pub fn set_physics_preset(&mut self, stiffness: f32, damping: f32) {
        self.width.apply_preset(stiffness, damping);
        self.height.apply_preset(stiffness, damping);
        self.top_offset.apply_preset(stiffness, damping);
    }

    pub fn step_all(&mut self, dt: f32) -> bool {
        let mut active = false;
        active |= self.width.step(dt);
        active |= self.height.step(dt);
        active |= self.top_offset.step(dt);
        active |= self.corner_radius.step(dt);
        active |= self.notch_flare.step(dt);
        active |= self.expand_progress.step(dt);
        active |= self.hover_scale.step(dt);
        active |= self.compact_alert_blend.step(dt);
        active |= self.page_scroll.step(dt);
        active |= self.osd_blend.step(dt);
        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges_smoothly_to_target() {
        let mut spring = Spring::bouncy(220.0);
        spring.set_target(440.0);
        let mut active = true;
        for _ in 0..300 {
            active = spring.step(1.0 / 120.0);
            if !active {
                break;
            }
        }
        assert!(!active, "Spring should settle within 2.5 seconds at 120fps");
        assert!((spring.value - 440.0).abs() < 0.01);
    }
}

