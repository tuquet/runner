use crate::drivers::cdp::connection::CdpConnection;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Simple fast non-cryptographic PRNG for humanized jitter and delays (Zero Dependency)
struct FastRng {
    state: AtomicU64,
}

impl FastRng {
    fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(88172645463325252);
        Self {
            state: AtomicU64::new(seed),
        }
    }

    fn next_u64(&self) -> u64 {
        let mut x = self.state.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state.store(x, Ordering::Relaxed);
        x
    }

    fn gen_range_f64(&self, min: f64, max: f64) -> f64 {
        let normalized = (self.next_u64() % 10000) as f64 / 10000.0;
        min + (max - min) * normalized
    }

    fn gen_range_u64(&self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() % (max - min))
    }
}

pub struct HumanInteractionEngine {
    connection: Arc<CdpConnection>,
    cursor_pos: Arc<RwLock<(f64, f64)>>,
    rng: FastRng,
}

impl HumanInteractionEngine {
    pub fn new(connection: Arc<CdpConnection>) -> Self {
        Self {
            connection,
            cursor_pos: Arc::new(RwLock::new((100.0, 100.0))),
            rng: FastRng::new(),
        }
    }

    /// Moves mouse pointer along a natural Cubic Bézier curve with speed variations
    pub async fn move_mouse(&self, target_x: f64, target_y: f64) -> Result<(), String> {
        let (start_x, start_y) = { *self.cursor_pos.read().await };
        let dx = target_x - start_x;
        let dy = target_y - start_y;
        let distance = (dx * dx + dy * dy).sqrt();

        // If mouse is already near target, minimal move
        if distance < 3.0 {
            self.dispatch_mouse_move(target_x, target_y).await?;
            let mut writer = self.cursor_pos.write().await;
            *writer = (target_x, target_y);
            return Ok(());
        }

        // Generate randomized Bézier control points
        let p0 = (start_x, start_y);
        let p3 = (target_x, target_y);

        let deviation = (distance * 0.25).min(80.0);
        let p1 = (
            start_x + dx * self.rng.gen_range_f64(0.2, 0.4) + self.rng.gen_range_f64(-deviation, deviation),
            start_y + dy * self.rng.gen_range_f64(0.1, 0.3) + self.rng.gen_range_f64(-deviation, deviation),
        );
        let p2 = (
            start_x + dx * self.rng.gen_range_f64(0.6, 0.8) + self.rng.gen_range_f64(-deviation, deviation),
            start_y + dy * self.rng.gen_range_f64(0.7, 0.9) + self.rng.gen_range_f64(-deviation, deviation),
        );

        // Calculate step count based on distance (15 to 40 steps)
        let steps = ((distance / 15.0).clamp(15.0, 40.0)) as usize;

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            // Ease-in-out cubic timing function
            let t_eased = if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            };

            // Cubic Bézier calculation
            let u = 1.0 - t_eased;
            let tt = t_eased * t_eased;
            let uu = u * u;
            let uuu = uu * u;
            let ttt = tt * t_eased;

            let cur_x = uuu * p0.0 + 3.0 * uu * t_eased * p1.0 + 3.0 * u * tt * p2.0 + ttt * p3.0;
            let cur_y = uuu * p0.1 + 3.0 * uu * t_eased * p1.1 + 3.0 * u * tt * p2.1 + ttt * p3.1;

            self.dispatch_mouse_move(cur_x, cur_y).await?;

            let sleep_ms = self.rng.gen_range_u64(3, 8);
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }

        // Final snap to target
        self.dispatch_mouse_move(target_x, target_y).await?;
        let mut writer = self.cursor_pos.write().await;
        *writer = (target_x, target_y);

        Ok(())
    }

    async fn dispatch_mouse_move(&self, x: f64, y: f64) -> Result<(), String> {
        self.connection
            .send_command(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mouseMoved",
                    "x": x.round(),
                    "y": y.round()
                }),
            )
            .await?;
        Ok(())
    }

    /// Natural human-like click sequence with randomized down/up dwell time
    pub async fn click_at(&self, x: f64, y: f64) -> Result<(), String> {
        self.move_mouse(x, y).await?;

        // Dwell pause before click
        let dwell_before = self.rng.gen_range_u64(30, 80);
        tokio::time::sleep(Duration::from_millis(dwell_before)).await;

        // mousePressed
        self.connection
            .send_command(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mousePressed",
                    "button": "left",
                    "clickCount": 1,
                    "x": x.round(),
                    "y": y.round()
                }),
            )
            .await?;

        // Dwell while pressed
        let dwell_pressed = self.rng.gen_range_u64(50, 110);
        tokio::time::sleep(Duration::from_millis(dwell_pressed)).await;

        // mouseReleased
        self.connection
            .send_command(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mouseReleased",
                    "button": "left",
                    "clickCount": 1,
                    "x": x.round(),
                    "y": y.round()
                }),
            )
            .await?;

        Ok(())
    }

    /// Human-like keystroke dispatching with randomized typing cadence
    pub async fn type_text(&self, text: &str) -> Result<(), String> {
        for ch in text.chars() {
            let char_str = ch.to_string();

            // keyDown
            self.connection
                .send_command(
                    "Input.dispatchKeyEvent",
                    json!({
                        "type": "keyDown",
                        "text": char_str,
                        "unmodifiedText": char_str
                    }),
                )
                .await?;

            // Hold key slightly
            let key_hold = self.rng.gen_range_u64(15, 35);
            tokio::time::sleep(Duration::from_millis(key_hold)).await;

            // keyUp
            self.connection
                .send_command(
                    "Input.dispatchKeyEvent",
                    json!({
                        "type": "keyUp"
                    }),
                )
                .await?;

            // Delay between letters
            let letter_delay = self.rng.gen_range_u64(35, 120);
            tokio::time::sleep(Duration::from_millis(letter_delay)).await;
        }

        Ok(())
    }

    /// Smooth human-like mouse wheel scrolling with deceleration momentum
    pub async fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<(), String> {
        let (cur_x, cur_y) = { *self.cursor_pos.read().await };

        let total_distance = (delta_x * delta_x + delta_y * delta_y).sqrt();
        let steps = ((total_distance / 40.0).clamp(5.0, 18.0)) as usize;

        let mut weights = Vec::with_capacity(steps);
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let w = (t * std::f64::consts::PI).sin().max(0.15);
            weights.push(w);
        }
        let weight_sum: f64 = weights.iter().sum();

        for w in weights {
            let ratio = w / weight_sum;
            let step_dx = delta_x * ratio;
            let step_dy = delta_y * ratio;

            self.connection
                .send_command(
                    "Input.dispatchMouseEvent",
                    json!({
                        "type": "mouseWheel",
                        "x": cur_x.round(),
                        "y": cur_y.round(),
                        "deltaX": step_dx.round(),
                        "deltaY": step_dy.round()
                    }),
                )
                .await?;

            let sleep_ms = self.rng.gen_range_u64(15, 35);
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }

        let settle_ms = self.rng.gen_range_u64(80, 160);
        tokio::time::sleep(Duration::from_millis(settle_ms)).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_rng_distribution() {
        let rng = FastRng::new();
        for _ in 0..100 {
            let val = rng.gen_range_f64(10.0, 50.0);
            assert!((10.0..=50.0).contains(&val));
            let int_val = rng.gen_range_u64(5, 20);
            assert!((5..20).contains(&int_val));
        }
    }
}
