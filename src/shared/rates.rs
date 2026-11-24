// SPDX-License-Identifier: GPL-3.0-only

//! Pure arithmetic on counters, shared by the system monitor and the control
//! panel's overview: busy percent from CPU ticks and watts from the RAPL
//! energy counters. No I/O, so each number is the same wherever it is shown.

/// Percent busy between two `(busy, total)` tick counters; 0 when no time passed.
pub fn busy_pct(prev: (u64, u64), now: (u64, u64)) -> f32 {
    let total = now.1.saturating_sub(prev.1);
    if total == 0 {
        0.0
    } else {
        now.0.saturating_sub(prev.0) as f32 / total as f32 * 100.0
    }
}

/// Energy used between two RAPL readings, in µJ. Each reading is
/// `(counter µJ, range where it wraps)`; a counter that went backwards wrapped.
pub fn rapl_used_uj(prev: &[(u64, u64)], now: &[(u64, u64)]) -> u64 {
    prev.iter()
        .zip(now)
        .map(|(&(a, range), &(b, _))| if b >= a { b - a } else { range - a + b })
        .sum()
}

/// Watts for `used` µJ over `dt_secs`.
pub fn uj_to_watts(used: u64, dt_secs: f32) -> f32 {
    used as f32 / 1e6 / dt_secs
}

/// Watts between two RAPL readings taken `dt_secs` apart; `None` without counters.
pub fn rapl_watts(prev: &[(u64, u64)], now: &[(u64, u64)], dt_secs: f32) -> Option<f32> {
    if prev.is_empty() || now.is_empty() {
        return None;
    }
    Some(uj_to_watts(rapl_used_uj(prev, now), dt_secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_pct_basic_and_zero() {
        assert_eq!(busy_pct((100, 1000), (150, 1100)), 50.0);
        assert_eq!(busy_pct((5, 5), (5, 5)), 0.0);
    }

    #[test]
    fn rapl_wraps() {
        let w = rapl_watts(&[(999_000_000, 1_000_000_000)], &[(1_000_000, 1_000_000_000)], 1.0).unwrap();
        assert!((w - 2.0).abs() < 1e-3); // 1 000 000 + 1 000 000 µJ = 2 J in 1 s
        assert!(rapl_watts(&[], &[], 1.0).is_none());
    }

    #[test]
    fn rapl_used_sums_domains_and_matches_watts() {
        let prev = [(10, 100), (500, 1000)];
        let now = [(30, 100), (400, 1000)]; // +20, then wrapped: 1000-500+400 = 900
        assert_eq!(rapl_used_uj(&prev, &now), 920);
        assert_eq!(rapl_used_uj(&[], &[]), 0);
        assert_eq!(rapl_watts(&prev, &now, 2.0), Some(uj_to_watts(920, 2.0)));
    }
}
