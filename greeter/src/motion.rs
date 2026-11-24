// Núcleo puro das animações do greeter/locker: constantes, curvas, `Motion<K>` e modo demo.
//
// Nada aqui toca a UI: `Motion` recebe instantes e devolve `Look`s (alfa e deslocamento) que
// as telas aplicam. Sem laços nem relógio próprio; os quadros só existem enquanto
// `is_animating()` (ver `frames`).

use cosmic::iced::animation::{Animation, Easing};
use cosmic::iced::time::every;
use cosmic::iced::Subscription;
use std::f32::consts::TAU;
use std::ffi::OsStr;
use std::time::{Duration, Instant};

pub const ANIMATION_TICK: Duration = Duration::from_millis(16);
pub const ENTRY_STAGGER: Duration = Duration::from_millis(40);
pub const ENTRY_ELEMENT: Duration = Duration::from_millis(180);
pub const ENTRY_TOTAL: Duration = Duration::from_millis(300);
pub const ENTRY_RISE: f32 = 12.0;
pub const CHROME_FADE: Duration = Duration::from_millis(250);
pub const ARM_TIMEOUT: Duration = Duration::from_secs(1);
pub const EXIT_FADE: Duration = Duration::from_millis(180);
pub const SHAKE_DURATION: Duration = Duration::from_millis(360);
pub const SHAKE_AMPLITUDE: f32 = 8.0;
pub const SHAKE_CYCLES: f32 = 3.0;
pub const HOVER: Duration = Duration::from_millis(120);
pub const FOCUS: Duration = Duration::from_millis(120);
pub const STATUS_FADE: Duration = Duration::from_millis(150);
pub const MENU_FADE: Duration = Duration::from_millis(140);
pub const MENU_RISE: f32 = 6.0;
pub const DEMO_STEP: Duration = Duration::from_millis(300);
pub const DEMO_ENV: &str = "COSMIC_GREETER_MOTION_DEMO";

/// Ease-out cúbico: `1 − (1 − p)³`, com `p` preso em `[0, 1]`.
pub fn ease_out_cubic(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    1.0 - (1.0 - p).powi(3)
}

/// Deslocamento horizontal do balanço de senha errada, `elapsed` após o início.
pub fn shake_dx(elapsed: Duration) -> f32 {
    if elapsed >= SHAKE_DURATION {
        return 0.0;
    }
    let p = (elapsed.as_secs_f32() / SHAKE_DURATION.as_secs_f32()).clamp(0.0, 1.0);
    SHAKE_AMPLITUDE * (1.0 - p).powi(2) * (TAU * SHAKE_CYCLES * p).sin()
}

/// Progresso (já com ease-out) de um trecho que começa em `delay` e dura `duration`.
fn progress(elapsed: Duration, delay: Duration, duration: Duration) -> f32 {
    let into = elapsed.saturating_sub(delay);
    if into >= duration {
        1.0
    } else {
        ease_out_cubic(into.as_secs_f32() / duration.as_secs_f32())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub alpha: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Look {
    pub const REST: Look = Look {
        alpha: 1.0,
        dx: 0.0,
        dy: 0.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Clock,
    Avatar,
    Name,
    Field,
    Status,
    Power,
}

impl Part {
    #[allow(dead_code)] // usados só nos testes
    pub const CASCADE: [Part; 4] = [Part::Clock, Part::Avatar, Part::Name, Part::Field];
    #[allow(dead_code)] // usados só nos testes
    pub const ALL: [Part; 6] = [
        Part::Clock,
        Part::Avatar,
        Part::Name,
        Part::Field,
        Part::Status,
        Part::Power,
    ];

    /// Posição na cascata de entrada (`Status` anda junto com o campo); `Power` fica de fora.
    fn cascade_index(self) -> Option<usize> {
        match self {
            Part::Clock => Some(0),
            Part::Avatar => Some(1),
            Part::Name => Some(2),
            Part::Field | Part::Status => Some(3),
            Part::Power => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverTarget {
    Field,
    Submit,
    Power,
}

impl HoverTarget {
    fn slot(self) -> usize {
        match self {
            HoverTarget::Field => 0,
            HoverTarget::Submit => 1,
            HoverTarget::Power => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    /// Em repouso: tudo visível.
    Idle,
    /// Esperando o primeiro quadro; conteúdo escondido até `start_entry` ou `ARM_TIMEOUT`.
    Armed(Instant),
    Running(Instant),
}

struct Hover<K> {
    /// `None` = todas as superfícies (só o modo demo).
    surface: Option<K>,
    anim: Animation<bool>,
}

fn bool_animation(duration: Duration) -> Animation<bool> {
    Animation::new(false)
        .duration(duration)
        .easing(Easing::EaseOutCubic)
}

pub struct Motion<K: Copy + PartialEq> {
    now: Instant,
    entry: Entry,
    exit: Option<Instant>,
    shake: Option<Instant>,
    menu: Option<Instant>,
    status_key: Option<u64>,
    status_start: Option<Instant>,
    focus: Animation<bool>,
    hovers: [Hover<K>; 3],
}

impl<K: Copy + PartialEq> Motion<K> {
    pub fn new(now: Instant) -> Self {
        let hover = || Hover {
            surface: None,
            anim: bool_animation(HOVER),
        };
        Self {
            now,
            entry: Entry::Idle,
            exit: None,
            shake: None,
            menu: None,
            status_key: None,
            status_start: None,
            focus: bool_animation(FOCUS),
            hovers: [hover(), hover(), hover()],
        }
    }

    /// Atribui `now` sem guarda de monotonicidade (o chamador passa instantes crescentes).
    pub fn tick(&mut self, now: Instant) {
        self.now = now;
    }

    pub fn now(&self) -> Instant {
        self.now
    }

    pub fn arm_entry(&mut self, now: Instant) {
        self.tick(now);
        self.entry = Entry::Armed(now);
    }

    /// Só age se a entrada estiver armada.
    pub fn start_entry(&mut self, now: Instant) {
        self.tick(now);
        if let Entry::Armed(armed) = self.entry {
            // Já passou do timeout: o conteúdo está visível pelo fallback; não recomeça.
            self.entry = Entry::Running(now.min(armed + ARM_TIMEOUT));
        }
    }

    /// Demo: a entrada recomeça já.
    pub fn restart_entry(&mut self, now: Instant) {
        self.tick(now);
        self.entry = Entry::Running(now);
    }

    /// `true` só na primeira chamada (até `cancel_exit`).
    pub fn start_exit(&mut self, now: Instant) -> bool {
        self.tick(now);
        if self.exit.is_some() {
            return false;
        }
        self.exit = Some(now);
        true
    }

    pub fn cancel_exit(&mut self) {
        self.exit = None;
    }

    pub fn exiting(&self) -> bool {
        self.exit.is_some()
    }

    /// O balanço só vale quando o erro veio de uma senha enviada.
    pub fn on_auth_error(&mut self, was_authenticating: bool, now: Instant) {
        self.tick(now);
        if was_authenticating {
            self.shake = Some(now);
        }
    }

    pub fn open_menu(&mut self, now: Instant) {
        self.tick(now);
        self.menu = Some(now);
    }

    /// Inicia o fade do status quando a chave muda.
    pub fn observe_status(&mut self, key: u64, now: Instant) {
        self.tick(now);
        if self.status_key != Some(key) {
            self.status_key = Some(key);
            self.status_start = Some(now);
        }
    }

    /// Grava a chave sem iniciar fade.
    pub fn set_status_key(&mut self, key: u64) {
        self.status_key = Some(key);
    }

    pub fn set_focus(&mut self, focused: bool, now: Instant) {
        self.tick(now);
        self.focus.go_mut(focused, now);
    }

    pub fn hover(&mut self, surface: K, target: HoverTarget, on: bool, now: Instant) {
        self.tick(now);
        let slot = &mut self.hovers[target.slot()];
        if on {
            slot.surface = Some(surface);
            slot.anim.go_mut(true, now);
        } else if slot.surface == Some(surface) {
            slot.anim.go_mut(false, now);
        }
    }

    /// O cursor saiu da superfície: apaga todos os hovers dela.
    pub fn cursor_left(&mut self, surface: K, now: Instant) {
        self.tick(now);
        for slot in &mut self.hovers {
            if slot.surface == Some(surface) {
                slot.anim.go_mut(false, now);
            }
        }
    }

    /// Demo: ⏻ em todas as superfícies.
    pub fn demo_hover(&mut self, on: bool, now: Instant) {
        self.tick(now);
        let slot = &mut self.hovers[HoverTarget::Power.slot()];
        slot.surface = None;
        slot.anim.go_mut(on, now);
    }

    fn entry_alpha_and_rise(&self, part: Part) -> (f32, f32) {
        let start = match self.entry {
            Entry::Idle => return (1.0, 0.0),
            Entry::Running(start) => start,
            Entry::Armed(armed) if self.now >= armed + ARM_TIMEOUT => armed + ARM_TIMEOUT,
            Entry::Armed(_) => {
                return (0.0, if part == Part::Power { 0.0 } else { ENTRY_RISE });
            }
        };
        let elapsed = self.now.saturating_duration_since(start);
        match part.cascade_index() {
            Some(i) => {
                let q = progress(elapsed, ENTRY_STAGGER * i as u32, ENTRY_ELEMENT);
                (q, ENTRY_RISE * (1.0 - q))
            }
            None => (progress(elapsed, Duration::ZERO, CHROME_FADE), 0.0),
        }
    }

    fn exit_alpha(&self) -> f32 {
        match self.exit {
            None => 1.0,
            Some(start) => {
                let elapsed = self.now.saturating_duration_since(start);
                1.0 - progress(elapsed, Duration::ZERO, EXIT_FADE)
            }
        }
    }

    fn status_alpha(&self) -> f32 {
        match self.status_start {
            None => 1.0,
            Some(start) => progress(
                self.now.saturating_duration_since(start),
                Duration::ZERO,
                STATUS_FADE,
            ),
        }
    }

    pub fn look(&self, part: Part) -> Look {
        let (mut alpha, dy) = self.entry_alpha_and_rise(part);
        alpha *= self.exit_alpha();
        if part == Part::Status {
            alpha *= self.status_alpha();
        }
        let dx = match (part, self.shake) {
            (Part::Field, Some(start)) => shake_dx(self.now.saturating_duration_since(start)),
            _ => 0.0,
        };
        Look { alpha, dx, dy }
    }

    pub fn menu_look(&self) -> Look {
        match self.menu {
            None => Look::REST,
            Some(start) => {
                let alpha = progress(
                    self.now.saturating_duration_since(start),
                    Duration::ZERO,
                    MENU_FADE,
                );
                Look {
                    alpha,
                    dx: 0.0,
                    dy: MENU_RISE * (1.0 - alpha),
                }
            }
        }
    }

    pub fn hover_t(&self, surface: K, target: HoverTarget) -> f32 {
        let slot = &self.hovers[target.slot()];
        match slot.surface {
            Some(owner) if owner != surface => 0.0,
            _ => slot.anim.interpolate(0.0, 1.0, self.now),
        }
    }

    pub fn focus_t(&self) -> f32 {
        self.focus.interpolate(0.0, 1.0, self.now)
    }

    fn within(&self, start: Option<Instant>, duration: Duration) -> bool {
        start.is_some_and(|s| self.now.saturating_duration_since(s) < duration)
    }

    pub fn is_animating(&self) -> bool {
        let entry = match self.entry {
            Entry::Idle => false,
            Entry::Armed(armed) => {
                self.now < armed + ARM_TIMEOUT
                    || self.within(Some(armed + ARM_TIMEOUT), ENTRY_TOTAL)
            }
            Entry::Running(start) => self.within(Some(start), ENTRY_TOTAL),
        };
        entry
            || self.within(self.exit, EXIT_FADE)
            || self.within(self.shake, SHAKE_DURATION)
            || self.within(self.menu, MENU_FADE)
            || self.within(self.status_start, STATUS_FADE)
            || self.focus.is_animating(self.now)
            || self.hovers.iter().any(|h| h.anim.is_animating(self.now))
    }

    pub fn frames(&self) -> Subscription<Instant> {
        if self.is_animating() {
            every(ANIMATION_TICK)
        } else {
            Subscription::none()
        }
    }

    pub fn debug_line(&self, epoch_ms: u128) -> String {
        let field = self.look(Part::Field);
        let hover = self.hovers[HoverTarget::Power.slot()]
            .anim
            .interpolate(0.0, 1.0, self.now);
        format!(
            "frame t={epoch_ms} clock={:.2} avatar={:.2} name={:.2} field={:.2} dy={:.2} dx={:.2} \
             exit={:.2} menu={:.2} status={:.2} hover={:.2}",
            self.look(Part::Clock).alpha,
            self.look(Part::Avatar).alpha,
            self.look(Part::Name).alpha,
            field.alpha,
            field.dy,
            field.dx,
            self.exit_alpha(),
            self.menu_look().alpha,
            self.look(Part::Status).alpha,
            hover,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoMode {
    All,
    Hover,
}

/// Modo demo a partir de `COSMIC_GREETER_MOTION_DEMO`; só liga no greeter/locker aninhado.
pub fn demo_mode_from(value: Option<&OsStr>, nested: bool) -> Option<DemoMode> {
    if !nested {
        return None;
    }
    match value?.to_str()? {
        "all" => Some(DemoMode::All),
        "hover" => Some(DemoMode::Hover),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoEvent {
    RestartEntry,
    Shake,
    Menu(bool),
    Hover(bool),
}

/// Roteiro do demo: ciclo de 10 passos de `DEMO_STEP`.
pub fn demo_event(slot: u64, mode: DemoMode) -> Option<DemoEvent> {
    match slot % 10 {
        0 if mode == DemoMode::All => Some(DemoEvent::RestartEntry),
        3 => Some(DemoEvent::Shake),
        5 => Some(DemoEvent::Menu(true)),
        7 => Some(DemoEvent::Menu(false)),
        8 => Some(DemoEvent::Hover(true)),
        9 => Some(DemoEvent::Hover(false)),
        _ => None,
    }
}

pub fn demo_ticks(mode: Option<DemoMode>) -> Subscription<Instant> {
    match mode {
        Some(_) => every(DEMO_STEP),
        None => Subscription::none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::advanced::subscription::into_recipes;
    use std::ffi::OsStr;

    fn ms(n: u64) -> Duration { Duration::from_millis(n) }
    fn entering(t0: Instant) -> Motion<u32> {
        let mut m = Motion::new(t0);
        m.arm_entry(t0);
        m.start_entry(t0);
        m
    }

    #[test]
    fn ease_out_cubic_endpoints_and_midpoint() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!((ease_out_cubic(0.5) - 0.875).abs() < 1e-6);
        assert_eq!(ease_out_cubic(-1.0), 0.0);
        assert_eq!(ease_out_cubic(2.0), 1.0);
    }
    #[test]
    fn idle_motion_is_at_rest_and_has_no_frames() {
        let m = Motion::<u32>::new(Instant::now());
        for p in Part::ALL { assert_eq!(m.look(p), Look::REST); }
        assert_eq!(m.menu_look(), Look::REST);
        assert!(!m.is_animating());
        assert_eq!(into_recipes(m.frames()).len(), 0);
    }
    #[test]
    fn animating_motion_subscribes_to_frames() {
        let m = entering(Instant::now());
        assert!(m.is_animating());
        assert_eq!(into_recipes(m.frames()).len(), 1);
    }
    #[test]
    fn entry_cascade_starts_hidden_and_low() {
        let m = entering(Instant::now());
        for p in Part::CASCADE {
            assert_eq!(m.look(p).alpha, 0.0);
            assert_eq!(m.look(p).dy, ENTRY_RISE);
        }
        assert_eq!(m.look(Part::Power).alpha, 0.0);
        assert_eq!(m.look(Part::Power).dy, 0.0);
    }
    #[test]
    fn entry_cascade_ends_at_rest_after_300ms() {
        let t0 = Instant::now();
        let mut m = entering(t0);
        assert_eq!(ENTRY_TOTAL, ms(300));
        m.tick(t0 + ENTRY_TOTAL);
        for p in Part::ALL { assert_eq!(m.look(p), Look::REST); }
        assert!(!m.is_animating());
    }
    #[test]
    fn cascade_order_clock_first() {
        let t0 = Instant::now();
        let mut m = entering(t0);
        for t in (0..=300).step_by(10) {
            m.tick(t0 + ms(t));
            let a = Part::CASCADE.map(|p| m.look(p).alpha);
            assert!(a.windows(2).all(|w| w[0] >= w[1]), "t={t} {a:?}");
        }
        m.tick(t0 + ms(120));
        assert!(m.look(Part::Clock).alpha > 0.9 && m.look(Part::Field).alpha == 0.0);
    }
    #[test]
    fn power_fades_in_250ms_without_rise() {
        let t0 = Instant::now();
        let mut m = entering(t0);
        m.tick(t0 + ms(125));
        let l = m.look(Part::Power);
        assert!(l.alpha > 0.0 && l.alpha < 1.0 && l.dy == 0.0);
        m.tick(t0 + CHROME_FADE);
        assert_eq!(m.look(Part::Power), Look::REST);
    }
    #[test]
    fn armed_entry_hides_content_but_starts_by_itself() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.arm_entry(t0);
        m.tick(t0 + ms(500));
        assert_eq!(m.look(Part::Field).alpha, 0.0);
        assert!(m.is_animating());
        m.tick(t0 + ARM_TIMEOUT + ENTRY_TOTAL);
        for p in Part::ALL { assert_eq!(m.look(p), Look::REST); }
        assert!(!m.is_animating());
    }
    #[test]
    fn late_start_entry_does_not_restart_cascade() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.arm_entry(t0);
        m.start_entry(t0 + ms(1300));
        assert_eq!(m.look(Part::Clock), Look::REST);
        assert!(!m.is_animating());
    }
    #[test]
    fn set_status_key_does_not_fade() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.set_status_key(1);
        assert_eq!(m.look(Part::Status).alpha, 1.0);
        assert!(!m.is_animating());
        m.observe_status(1, t0);
        assert_eq!(m.look(Part::Status).alpha, 1.0);
    }
    #[test]
    fn restart_entry_begins_now() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.restart_entry(t0);
        assert_eq!(m.look(Part::Clock).alpha, 0.0);
        assert!(m.is_animating());
        m.tick(t0 + ENTRY_TOTAL);
        assert_eq!(m.look(Part::Clock), Look::REST);
    }
    #[test]
    fn start_entry_without_arm_is_noop() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.start_entry(t0);
        assert_eq!(m.look(Part::Clock), Look::REST);
        assert!(!m.is_animating());
    }
    #[test]
    fn exit_fade_reaches_zero_at_180ms() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        assert_eq!(EXIT_FADE, ms(180));
        assert!(m.start_exit(t0));
        m.tick(t0 + ms(90));
        let a = m.look(Part::Field).alpha;
        assert!(a > 0.0 && a < 1.0);
        m.tick(t0 + EXIT_FADE);
        for p in Part::ALL { assert_eq!(m.look(p).alpha, 0.0); }
        assert!(m.exiting());
        assert!(!m.is_animating());
    }
    #[test]
    fn start_exit_is_idempotent() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        assert!(m.start_exit(t0));
        assert!(!m.start_exit(t0 + ms(50)));
        m.tick(t0 + EXIT_FADE);
        assert_eq!(m.look(Part::Clock).alpha, 0.0);
    }
    #[test]
    fn cancel_exit_restores_content() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.start_exit(t0);
        m.tick(t0 + EXIT_FADE);
        m.cancel_exit();
        assert!(!m.exiting());
        for p in Part::ALL { assert_eq!(m.look(p), Look::REST); }
    }
    #[test]
    fn relock_resets_exit_and_rearms_entry() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.start_exit(t0);
        let t1 = t0 + Duration::from_secs(60);
        m.tick(t1);
        m.cancel_exit();
        m.arm_entry(t1);
        assert_eq!(m.look(Part::Clock).alpha, 0.0);
        assert!(m.is_animating());
        m.start_entry(t1);
        m.tick(t1 + ENTRY_TOTAL);
        assert_eq!(m.look(Part::Clock), Look::REST);
    }
    #[test]
    fn shake_only_after_password_submit() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.on_auth_error(false, t0);
        m.tick(t0 + ms(30));
        assert_eq!(m.look(Part::Field).dx, 0.0);
        assert!(!m.is_animating());
        m.on_auth_error(true, t0 + ms(30));
        m.tick(t0 + ms(60));
        assert!(m.look(Part::Field).dx != 0.0);
        assert_eq!(m.look(Part::Avatar).dx, 0.0);
    }
    #[test]
    fn shake_is_damped_and_settles() {
        assert!(SHAKE_DURATION >= ms(300) && SHAKE_DURATION <= ms(400));
        assert_eq!(shake_dx(ms(0)), 0.0);
        assert_eq!(shake_dx(SHAKE_DURATION), 0.0);
        let peak = |a: u64, b: u64| (a..b).map(|t| shake_dx(ms(t)).abs()).fold(0.0, f32::max);
        for t in 0..=360 { assert!(shake_dx(ms(t)).abs() <= SHAKE_AMPLITUDE); }
        assert!(peak(0, 120) > peak(240, 360));
        assert!(peak(0, 120) > 3.0);
    }
    #[test]
    fn hover_is_per_surface() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.hover(1, HoverTarget::Power, true, t0);
        m.tick(t0 + HOVER + ms(1));
        assert_eq!(m.hover_t(1, HoverTarget::Power), 1.0);
        assert_eq!(m.hover_t(2, HoverTarget::Power), 0.0);
        assert_eq!(m.hover_t(1, HoverTarget::Submit), 0.0);
    }
    #[test]
    fn cursor_left_clears_hover_of_that_surface() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.hover(1, HoverTarget::Field, true, t0);
        m.hover(1, HoverTarget::Submit, true, t0);
        let t1 = t0 + HOVER + ms(1);
        m.tick(t1);
        m.cursor_left(1, t1);
        m.tick(t1 + HOVER + ms(1));
        assert_eq!(m.hover_t(1, HoverTarget::Field), 0.0);
        assert_eq!(m.hover_t(1, HoverTarget::Submit), 0.0);
    }
    #[test]
    fn held_hover_stops_frames() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.hover(1, HoverTarget::Power, true, t0);
        assert!(m.is_animating());
        m.tick(t0 + HOVER + ms(1));
        assert!(!m.is_animating());
        assert_eq!(m.hover_t(1, HoverTarget::Power), 1.0);
    }
    #[test]
    fn focus_transition_120ms() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.set_focus(true, t0);
        m.tick(t0 + ms(60));
        assert!(m.focus_t() > 0.5 && m.focus_t() < 1.0);
        m.tick(t0 + FOCUS + ms(1));
        assert_eq!(m.focus_t(), 1.0);
    }
    #[test]
    fn status_fades_in_on_change_only() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.observe_status(1, t0);
        m.tick(t0 + ms(75));
        let a = m.look(Part::Status).alpha;
        assert!(a > 0.0 && a < 1.0);
        m.observe_status(1, t0 + ms(100));
        m.tick(t0 + STATUS_FADE);
        assert_eq!(m.look(Part::Status).alpha, 1.0);
        assert!(!m.is_animating());
        m.observe_status(2, t0 + STATUS_FADE);
        assert_eq!(m.look(Part::Status).alpha, 0.0);
    }
    #[test]
    fn menu_fades_and_rises() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        m.open_menu(t0);
        assert_eq!(m.menu_look(), Look { alpha: 0.0, dx: 0.0, dy: MENU_RISE });
        m.tick(t0 + MENU_FADE);
        assert_eq!(m.menu_look(), Look::REST);
    }
    #[test]
    fn debug_line_reports_values() {
        let m = Motion::<u32>::new(Instant::now());
        let line = m.debug_line(42);
        assert!(line.starts_with("frame t=42 "));
        assert!(line.contains(" exit=1.00 ") && line.contains(" field=1.00 "));
    }
    #[test]
    fn demo_needs_nested_and_known_value() {
        assert_eq!(demo_mode_from(Some(OsStr::new("all")), true), Some(DemoMode::All));
        assert_eq!(demo_mode_from(Some(OsStr::new("hover")), true), Some(DemoMode::Hover));
        assert_eq!(demo_mode_from(Some(OsStr::new("all")), false), None);
        assert_eq!(demo_mode_from(Some(OsStr::new("x")), true), None);
        assert_eq!(demo_mode_from(None, true), None);
        assert_eq!(into_recipes(demo_ticks(None)).len(), 0);
    }
    #[test]
    fn demo_schedule_cycles_every_ten_slots() {
        assert_eq!(demo_event(0, DemoMode::All), Some(DemoEvent::RestartEntry));
        assert_eq!(demo_event(0, DemoMode::Hover), None);
        assert_eq!(demo_event(1, DemoMode::All), None);
        assert_eq!(demo_event(3, DemoMode::Hover), Some(DemoEvent::Shake));
        assert_eq!(demo_event(5, DemoMode::All), Some(DemoEvent::Menu(true)));
        assert_eq!(demo_event(7, DemoMode::All), Some(DemoEvent::Menu(false)));
        assert_eq!(demo_event(8, DemoMode::All), Some(DemoEvent::Hover(true)));
        assert_eq!(demo_event(9, DemoMode::All), Some(DemoEvent::Hover(false)));
        assert_eq!(demo_event(10, DemoMode::All), Some(DemoEvent::RestartEntry));
    }
}
