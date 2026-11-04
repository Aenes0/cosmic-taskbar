// SPDX-License-Identifier: GPL-3.0-only
//! Máquina de estados pura dos popups da bandeja (caixa de overflow e menus).
//!
//! O libcosmic desiste em silêncio de popups pedidos enquanto outro está fechando, e destruir
//! um id inexistente cancela o pedido pendente. Por isso: só o `opened(id)` confirma, nunca se
//! destrói o que não está aberto, e pedidos esperam numa fila de 1 enquanto algo fecha.

use std::time::{Duration, Instant};

use cosmic::iced::window;

/// Tempo máximo à espera de `opened` / `closed` antes de dar o pedido por perdido / o popup por fechado.
pub const LOST: Duration = Duration::from_millis(300);
/// Quantos pedidos perdidos lembrar para o caso de um `Opened` atrasado.
const LOST_KEEP: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Want {
    Box,
    Menu { item: usize, in_box: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Parent {
    Main,
    Box(window::Id),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    Open { want: Want, id: window::Id, parent: Parent },
    Destroy(window::Id),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Requested,
    Open,
    Closing,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    id: window::Id,
    phase: Phase,
    since: Instant,
    /// Fechar assim que abrir (Esc durante o pedido).
    dismiss: bool,
}

#[derive(Clone, Copy, Debug)]
struct MenuSlot {
    item: usize,
    in_box: bool,
    slot: Slot,
}

#[derive(Clone, Copy, Debug)]
struct Lost {
    id: window::Id,
    want: Want,
    seq: u64,
}

#[derive(Default)]
pub struct Popups {
    box_: Option<Slot>,
    menu: Option<MenuSlot>,
    /// Vontade pendente (a última vence); sai quando nada mais está fechando.
    queue: Option<Want>,
    /// Destruídos por adoção negada; contam como "fechando" até o `closed`.
    orphans: Vec<(window::Id, Instant)>,
    lost: Vec<Lost>,
    /// Contador de `Open` emitidos.
    seq: u64,
}

impl Popups {
    pub fn box_id(&self) -> Option<window::Id> {
        self.box_.filter(|s| s.phase != Phase::Closing).map(|s| s.id)
    }

    pub fn menu_item(&self) -> Option<(usize, window::Id)> {
        self.menu
            .filter(|m| m.slot.phase != Phase::Closing)
            .map(|m| (m.item, m.slot.id))
    }

    pub fn is_box_open(&self) -> bool {
        matches!(self.box_, Some(Slot { phase: Phase::Open, .. }))
    }

    pub fn pending(&self) -> bool {
        self.queue.is_some()
            || !self.orphans.is_empty()
            || self.box_.is_some_and(|s| s.phase != Phase::Open)
            || self.menu.is_some_and(|m| m.slot.phase != Phase::Open)
    }

    pub fn toggle_box(&mut self, now: Instant, fresh: window::Id) -> Vec<Cmd> {
        self.expire(now);
        let mut cmds = Vec::new();
        match self.box_.map(|s| s.phase) {
            Some(Phase::Requested) => return cmds,
            Some(Phase::Open) => {
                self.queue = None;
                self.destroy_box(now, &mut cmds);
                return cmds;
            }
            Some(Phase::Closing) | None => {
                self.queue = if self.queue == Some(Want::Box) { None } else { Some(Want::Box) };
            }
        }
        self.settle(now, fresh, &mut cmds);
        cmds
    }

    pub fn menu(&mut self, item: usize, in_box: bool, now: Instant, fresh: window::Id) -> Vec<Cmd> {
        self.expire(now);
        let want = Want::Menu { item, in_box };
        let mut cmds = Vec::new();
        if let Some(m) = self.menu.filter(|m| m.item == item && m.in_box == in_box) {
            match m.slot.phase {
                Phase::Requested => return cmds,
                Phase::Open => {
                    self.queue = None;
                    self.destroy_menu(now, &mut cmds);
                    return cmds;
                }
                Phase::Closing => {
                    self.queue = if self.queue == Some(want) { None } else { Some(want) };
                    return cmds;
                }
            }
        }
        // Menu da caixa só com a caixa aberta (ou pedida, esperando o `opened`).
        if in_box && !matches!(self.box_.map(|s| s.phase), Some(Phase::Open | Phase::Requested)) {
            return cmds;
        }
        self.queue = Some(want);
        self.settle(now, fresh, &mut cmds);
        cmds
    }

    pub fn escape(&mut self, now: Instant) -> Vec<Cmd> {
        self.expire(now);
        let mut cmds = Vec::new();
        if matches!(self.queue, Some(Want::Menu { .. })) {
            self.queue = None;
        }
        match self.menu.map(|m| m.slot.phase) {
            Some(Phase::Open) => self.destroy_menu(now, &mut cmds),
            Some(Phase::Requested) => {
                if let Some(m) = self.menu.as_mut() {
                    m.slot.dismiss = true;
                }
            }
            _ => {}
        }
        cmds
    }

    pub fn opened(&mut self, id: window::Id, now: Instant) -> Vec<Cmd> {
        self.expire(now);
        let mut cmds = Vec::new();
        if let Some(s) = self.box_.as_mut().filter(|s| s.id == id && s.phase == Phase::Requested) {
            s.phase = Phase::Open;
            s.since = now;
        } else if self.menu.is_some_and(|m| m.slot.id == id && m.slot.phase == Phase::Requested) {
            if let Some(m) = self.menu.as_mut() {
                m.slot.phase = Phase::Open;
                m.slot.since = now;
            }
            if self.menu.is_some_and(|m| m.slot.dismiss) {
                self.destroy_menu(now, &mut cmds);
            }
        } else if let Some(pos) = self.lost.iter().position(|l| l.id == id) {
            let l = self.lost.remove(pos);
            self.adopt_or_destroy(l, now, &mut cmds);
        }
        // Sem `fresh` aqui: a fila só abre de novo no `closed`/`tick`; mas pode precisar
        // destruir (ex.: algo esperava este pedido confirmar).
        self.settle_destroy_only(now, &mut cmds);
        cmds
    }

    pub fn closed(&mut self, id: window::Id, now: Instant, fresh: window::Id) -> Vec<Cmd> {
        self.expire(now);
        let mut cmds = Vec::new();
        if self.box_.is_some_and(|s| s.id == id) {
            // Fechar a caixa leva o menu filho junto, sem destroy.
            self.box_ = None;
            if self.menu.is_some_and(|m| m.in_box) {
                self.menu = None;
            }
        } else if self.menu.is_some_and(|m| m.slot.id == id) {
            self.menu = None;
        }
        self.orphans.retain(|(o, _)| *o != id);
        self.lost.retain(|l| l.id != id);
        self.settle(now, fresh, &mut cmds);
        cmds
    }

    pub fn tick(&mut self, now: Instant, fresh: window::Id) -> Vec<Cmd> {
        self.expire(now);
        let mut cmds = Vec::new();
        self.settle(now, fresh, &mut cmds);
        cmds
    }

    // ----- internos -----

    fn closing_any(&self) -> bool {
        self.box_.is_some_and(|s| s.phase == Phase::Closing)
            || self.menu.is_some_and(|m| m.slot.phase == Phase::Closing)
            || !self.orphans.is_empty()
    }

    fn expired(since: Instant, now: Instant) -> bool {
        now.saturating_duration_since(since) >= LOST
    }

    fn remember_lost(&mut self, id: window::Id, want: Want, seq_at_open: u64) {
        self.lost.push(Lost { id, want, seq: seq_at_open });
        if self.lost.len() > LOST_KEEP {
            self.lost.remove(0);
        }
    }

    fn expire(&mut self, now: Instant) {
        if let Some(s) = self.box_ {
            if Self::expired(s.since, now) {
                match s.phase {
                    Phase::Requested => {
                        let seq = self.seq;
                        self.remember_lost(s.id, Want::Box, seq);
                        self.box_ = None;
                        if self.menu.is_some_and(|m| m.in_box) {
                            self.menu = None;
                        }
                    }
                    Phase::Closing => {
                        self.box_ = None;
                        if self.menu.is_some_and(|m| m.in_box) {
                            self.menu = None;
                        }
                    }
                    Phase::Open => {}
                }
            }
        }
        if let Some(m) = self.menu {
            if Self::expired(m.slot.since, now) {
                match m.slot.phase {
                    Phase::Requested => {
                        let seq = self.seq;
                        self.remember_lost(m.slot.id, Want::Menu { item: m.item, in_box: m.in_box }, seq);
                        self.menu = None;
                    }
                    Phase::Closing => self.menu = None,
                    Phase::Open => {}
                }
            }
        }
        self.orphans.retain(|(_, since)| !Self::expired(*since, now));
    }

    fn destroy_box(&mut self, now: Instant, cmds: &mut Vec<Cmd>) {
        if let Some(s) = self.box_.as_mut().filter(|s| s.phase == Phase::Open) {
            s.phase = Phase::Closing;
            s.since = now;
            cmds.push(Cmd::Destroy(s.id));
            // O menu filho sai do estado junto com a caixa.
            if self.menu.is_some_and(|m| m.in_box) {
                self.menu = None;
            }
        }
    }

    fn destroy_menu(&mut self, now: Instant, cmds: &mut Vec<Cmd>) {
        if let Some(m) = self.menu.as_mut().filter(|m| m.slot.phase == Phase::Open) {
            m.slot.phase = Phase::Closing;
            m.slot.since = now;
            cmds.push(Cmd::Destroy(m.slot.id));
        }
    }

    fn adopt_or_destroy(&mut self, l: Lost, now: Instant, cmds: &mut Vec<Cmd>) {
        let nothing_after = l.seq == self.seq && self.queue.is_none();
        let adopt = nothing_after
            && match l.want {
                Want::Box => self.box_.is_none(),
                Want::Menu { in_box, .. } => {
                    self.menu.is_none()
                        && (!in_box || self.is_box_open())
                        && (in_box || self.box_.is_none())
                }
            };
        let slot = Slot { id: l.id, phase: Phase::Open, since: now, dismiss: false };
        if adopt {
            match l.want {
                Want::Box => self.box_ = Some(slot),
                Want::Menu { item, in_box } => self.menu = Some(MenuSlot { item, in_box, slot }),
            }
        } else {
            self.orphans.push((l.id, now));
            cmds.push(Cmd::Destroy(l.id));
        }
    }

    /// Para o `opened`: só destrói (sem id novo disponível), p. ex. caixa recém-aberta
    /// com um menu da barra esperando.
    fn settle_destroy_only(&mut self, now: Instant, cmds: &mut Vec<Cmd>) {
        // Id novo e único: se `settle` abrir algo, o id é válido.
        self.settle(now, window::Id::unique(), cmds);
    }

    fn settle(&mut self, now: Instant, fresh: window::Id, cmds: &mut Vec<Cmd>) {
        let Some(want) = self.queue else { return };
        if self.closing_any() {
            return;
        }
        let box_phase = self.box_.map(|s| s.phase);
        match want {
            Want::Box => {
                if self.menu.is_some() {
                    self.clear_menu_for(now, cmds);
                    return;
                }
                if self.box_.is_some() {
                    self.queue = None;
                    return;
                }
                self.open(want, fresh, Parent::Main, now, cmds);
            }
            Want::Menu { item, in_box } => {
                if in_box {
                    match box_phase {
                        Some(Phase::Open) => {}
                        Some(Phase::Requested) => return,
                        _ => {
                            self.queue = None;
                            return;
                        }
                    }
                } else if let Some(phase) = box_phase {
                    if phase == Phase::Open {
                        self.destroy_box(now, cmds);
                    }
                    return;
                }
                if let Some(m) = self.menu {
                    if m.item == item && m.in_box == in_box {
                        self.queue = None;
                    } else {
                        self.clear_menu_for(now, cmds);
                    }
                    return;
                }
                let parent = match (in_box, self.box_) {
                    (true, Some(b)) => Parent::Box(b.id),
                    _ => Parent::Main,
                };
                self.open(want, fresh, parent, now, cmds);
            }
        }
    }

    /// Fecha o menu existente (se aberto); se apenas pedido, espera a confirmação.
    fn clear_menu_for(&mut self, now: Instant, cmds: &mut Vec<Cmd>) {
        if self.menu.is_some_and(|m| m.slot.phase == Phase::Open) {
            self.destroy_menu(now, cmds);
        }
    }

    fn open(&mut self, want: Want, id: window::Id, parent: Parent, now: Instant, cmds: &mut Vec<Cmd>) {
        self.queue = None;
        self.seq += 1;
        let slot = Slot { id, phase: Phase::Requested, since: now, dismiss: false };
        match want {
            Want::Box => self.box_ = Some(slot),
            Want::Menu { item, in_box } => self.menu = Some(MenuSlot { item, in_box, slot }),
        }
        cmds.push(Cmd::Open { want, id, parent });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> window::Id {
        window::Id::unique()
    }
    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }
    fn open_box(p: &mut Popups, t: Instant) -> window::Id {
        let f = id();
        assert_eq!(
            p.toggle_box(t, f),
            vec![Cmd::Open { want: Want::Box, id: f, parent: Parent::Main }]
        );
        assert!(p.opened(f, t).is_empty());
        f
    }
    fn is_destroy(c: &[Cmd]) -> bool {
        c.iter().any(|x| matches!(x, Cmd::Destroy(_)))
    }

    #[test]
    fn toggle_requests_box_but_only_opened_confirms() {
        let t = Instant::now();
        let mut p = Popups::default();
        let f = id();
        assert_eq!(p.toggle_box(t, f).len(), 1);
        assert_eq!(p.box_id(), Some(f));
        assert!(!p.is_box_open());
        assert!(p.pending());
        p.opened(f, t);
        assert!(p.is_box_open());
        assert!(!p.pending());
    }

    #[test]
    fn toggle_on_open_box_destroys_it() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        assert_eq!(p.toggle_box(ms(t, 10), id()), vec![Cmd::Destroy(b)]);
        assert_eq!(p.box_id(), None);
        assert!(p.closed(b, ms(t, 20), id()).is_empty());
        assert!(!p.pending());
    }

    #[test]
    fn foco1_second_click_before_opened_does_nothing() {
        let t = Instant::now();
        let mut p = Popups::default();
        let f = id();
        p.toggle_box(t, f);
        assert!(p.toggle_box(ms(t, 5), id()).is_empty());
        assert!(!p.is_box_open());
        p.opened(f, ms(t, 20));
        assert!(p.is_box_open());
    }

    #[test]
    fn foco2_lost_request_returns_to_closed_and_next_click_opens() {
        let t = Instant::now();
        let mut p = Popups::default();
        p.toggle_box(t, id());
        assert!(p.tick(ms(t, 299), id()).is_empty());
        assert!(p.box_id().is_some());
        assert!(p.tick(ms(t, 300), id()).is_empty());
        assert_eq!(p.box_id(), None);
        assert!(!p.pending());
        let f = id();
        let cmds = p.toggle_box(ms(t, 310), f);
        assert_eq!(cmds, vec![Cmd::Open { want: Want::Box, id: f, parent: Parent::Main }]);
    }

    #[test]
    fn never_destroys_what_is_not_open() {
        let t = Instant::now();
        let mut p = Popups::default();
        assert!(p.escape(t).is_empty());
        assert!(p.closed(id(), t, id()).is_empty());
        let f = id();
        p.toggle_box(t, f);
        assert!(!is_destroy(&p.toggle_box(ms(t, 1), id())));
        assert!(!is_destroy(&p.escape(ms(t, 2))));
        p.tick(ms(t, 400), id());
        assert!(!is_destroy(&p.toggle_box(ms(t, 401), id())));
    }

    #[test]
    fn click_while_closing_is_queued_and_opens_on_closed() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        p.toggle_box(ms(t, 10), id());
        assert!(p.toggle_box(ms(t, 20), id()).is_empty());
        assert!(p.pending());
        let f = id();
        assert_eq!(
            p.closed(b, ms(t, 30), f),
            vec![Cmd::Open { want: Want::Box, id: f, parent: Parent::Main }]
        );
    }

    #[test]
    fn last_wish_wins_in_queue() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        p.toggle_box(ms(t, 10), id());
        p.toggle_box(ms(t, 11), id()); // fila: Box
        assert!(p.menu(3, false, ms(t, 12), id()).is_empty()); // fila: Menu (vence)
        let f = id();
        assert_eq!(
            p.closed(b, ms(t, 20), f),
            vec![Cmd::Open { want: Want::Menu { item: 3, in_box: false }, id: f, parent: Parent::Main }]
        );
    }

    #[test]
    fn closing_without_closed_is_assumed_closed_after_lost() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        p.toggle_box(ms(t, 10), id());
        p.toggle_box(ms(t, 20), id());
        assert!(p.tick(ms(t, 100), id()).is_empty());
        let f = id();
        let cmds = p.tick(ms(t, 10 + 300), f);
        assert_eq!(cmds, vec![Cmd::Open { want: Want::Box, id: f, parent: Parent::Main }]);
        let _ = b;
    }

    #[test]
    fn foco3_compositor_close_clears_box_and_menu_without_destroy() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        let m = id();
        p.menu(1, true, ms(t, 5), m);
        p.opened(m, ms(t, 6));
        assert_eq!(p.menu_item(), Some((1, m)));
        assert!(p.closed(b, ms(t, 10), id()).is_empty());
        assert_eq!(p.box_id(), None);
        assert_eq!(p.menu_item(), None);
        assert!(p.closed(m, ms(t, 11), id()).is_empty());
        assert!(p.tick(ms(t, 500), id()).is_empty());
        assert!(!p.pending());
    }

    #[test]
    fn foco4_destroying_box_removes_child_menu_from_state() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        let m = id();
        let cmds = p.menu(2, true, ms(t, 5), m);
        assert_eq!(cmds, vec![Cmd::Open { want: Want::Menu { item: 2, in_box: true }, id: m, parent: Parent::Box(b) }]);
        p.opened(m, ms(t, 6));
        assert_eq!(p.toggle_box(ms(t, 10), id()), vec![Cmd::Destroy(b)]);
        assert_eq!(p.menu_item(), None);
        assert!(p.closed(b, ms(t, 20), id()).is_empty());
        assert!(!p.pending());
    }

    #[test]
    fn foco5_late_opened_of_lost_request_is_adopted_if_nothing_after() {
        let t = Instant::now();
        let mut p = Popups::default();
        let f = id();
        p.toggle_box(t, f);
        p.tick(ms(t, 300), id());
        assert!(p.opened(f, ms(t, 350)).is_empty());
        assert!(p.is_box_open());
        assert_eq!(p.box_id(), Some(f));
    }

    #[test]
    fn foco5_late_opened_is_destroyed_if_something_was_requested_after() {
        let t = Instant::now();
        let mut p = Popups::default();
        let old = id();
        p.toggle_box(t, old);
        p.tick(ms(t, 300), id());
        let newer = id();
        p.toggle_box(ms(t, 310), newer);
        assert_eq!(p.opened(old, ms(t, 320)), vec![Cmd::Destroy(old)]);
        assert_eq!(p.box_id(), Some(newer));
        // o órfão em fechamento bloqueia aberturas novas até o `closed`
        p.opened(newer, ms(t, 321));
        assert!(p.pending());
        assert!(p.closed(old, ms(t, 330), id()).is_empty());
    }

    #[test]
    fn bar_menu_with_box_open_destroys_box_then_opens_menu_on_closed() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        assert_eq!(p.menu(4, false, ms(t, 10), id()), vec![Cmd::Destroy(b)]);
        let m = id();
        assert_eq!(
            p.closed(b, ms(t, 20), m),
            vec![Cmd::Open { want: Want::Menu { item: 4, in_box: false }, id: m, parent: Parent::Main }]
        );
    }

    #[test]
    fn box_menu_requires_open_box() {
        let t = Instant::now();
        let mut p = Popups::default();
        assert!(p.menu(1, true, t, id()).is_empty());
        assert_eq!(p.menu_item(), None);
        assert!(!p.pending());
    }

    #[test]
    fn same_item_closes_and_other_item_switches() {
        let t = Instant::now();
        let mut p = Popups::default();
        let m = id();
        p.menu(1, false, t, m);
        p.opened(m, ms(t, 1));
        assert_eq!(p.menu(1, false, ms(t, 5), id()), vec![Cmd::Destroy(m)]);
        assert!(p.closed(m, ms(t, 6), id()).is_empty());

        let m2 = id();
        p.menu(1, false, ms(t, 10), m2);
        p.opened(m2, ms(t, 11));
        assert_eq!(p.menu(2, false, ms(t, 15), id()), vec![Cmd::Destroy(m2)]);
        let m3 = id();
        assert_eq!(
            p.closed(m2, ms(t, 16), m3),
            vec![Cmd::Open { want: Want::Menu { item: 2, in_box: false }, id: m3, parent: Parent::Main }]
        );
    }

    #[test]
    fn escape_closes_only_the_menu() {
        let t = Instant::now();
        let mut p = Popups::default();
        let b = open_box(&mut p, t);
        let m = id();
        p.menu(1, true, ms(t, 1), m);
        p.opened(m, ms(t, 2));
        assert_eq!(p.escape(ms(t, 3)), vec![Cmd::Destroy(m)]);
        assert_eq!(p.box_id(), Some(b));
        assert!(p.is_box_open());
    }
}
