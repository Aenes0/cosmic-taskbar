// SPDX-License-Identifier: GPL-3.0-only

//! Avisos tipados (toasts) e histórico de mudanças recentes do painel.

use std::time::{Duration, Instant};

use super::Section;

pub const TOAST_TTL: Duration = Duration::from_secs(5);
pub const MAX_VISIBLE: usize = 3;
const MAX_RECENT: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
    Progress,
}

#[derive(Clone, Debug)]
pub struct Toast<M> {
    pub id: u64,
    pub kind: ToastKind,
    pub text: String,
    pub undo: Option<M>,
    pub born: Instant,
}

pub struct Toasts<M> {
    items: Vec<Toast<M>>,
    next_id: u64,
}

impl<M> Default for Toasts<M> {
    fn default() -> Self {
        Self { items: Vec::new(), next_id: 0 }
    }
}

impl<M: Clone> Toasts<M> {
    pub fn push(&mut self, kind: ToastKind, text: impl Into<String>, undo: Option<M>, now: Instant) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Toast { id, kind, text: text.into(), undo, born: now });
        // Política de rajada: acima de MAX_VISIBLE a vítima é escolhida entre os ANTIGOS (nunca o
        // recém-chegado): o Success/Info mais antigo; sem nenhum, o mais antigo de todos.
        while self.items.len() > MAX_VISIBLE {
            let old = self.items.len() - 1;
            let victim = self.items[..old]
                .iter()
                .position(|t| matches!(t.kind, ToastKind::Success | ToastKind::Info))
                .unwrap_or(0);
            self.items.remove(victim);
        }
        id
    }

    pub fn dismiss(&mut self, id: u64) {
        self.items.retain(|t| t.id != id);
    }

    pub fn expire(&mut self, now: Instant) {
        self.items.retain(|t| match t.kind {
            ToastKind::Success | ToastKind::Info => now.saturating_duration_since(t.born) <= TOAST_TTL,
            ToastKind::Error | ToastKind::Progress => true,
        });
    }

    pub fn visible(&self) -> &[Toast<M>] {
        &self.items
    }
}

pub fn kind_from_legacy(text: &str) -> ToastKind {
    let t = text.trim_start();
    const FAILURE_MARKERS: [&str; 5] =
        ["Cancelado", "retornou código", "Nenhum terminal", "falhou", "não foi encontrado"];
    if t.starts_with('✗')
        || t.starts_with("Falha")
        || t.starts_with("Erro")
        || FAILURE_MARKERS.iter().any(|m| t.contains(m))
    {
        ToastKind::Error
    } else if t.starts_with('✓') {
        ToastKind::Success
    } else {
        ToastKind::Info
    }
}

#[derive(Clone, Debug)]
pub struct RecentChange<M> {
    pub label: String,
    pub section: Section,
    pub at: Instant,
    pub undo: Option<M>,
}

pub struct Recent<M> {
    items: Vec<RecentChange<M>>,
}

impl<M> Default for Recent<M> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<M: Clone> Recent<M> {
    pub fn record(&mut self, c: RecentChange<M>) {
        self.items.insert(0, c);
        self.items.truncate(MAX_RECENT);
    }

    pub fn items(&self) -> &[RecentChange<M>] {
        &self.items
    }

    pub fn take_undo(&mut self, idx: usize) -> Option<M> {
        self.items.get_mut(idx).and_then(|c| c.undo.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_keeps_three_and_errors_stay() {
        let t0 = Instant::now();
        let mut q = Toasts::<u8>::default();
        q.push(ToastKind::Error, "Falha x", None, t0);
        for i in 0..5 {
            q.push(ToastKind::Success, format!("ok {i}"), None, t0);
        }
        assert_eq!(q.visible().len(), MAX_VISIBLE);
        assert!(q.visible().iter().any(|t| matches!(t.kind, ToastKind::Error)));
        q.expire(t0 + TOAST_TTL + Duration::from_millis(1));
        assert_eq!(q.visible().len(), 1);
    }

    #[test]
    fn newcomer_success_survives_three_errors() {
        let t0 = Instant::now();
        let mut q = Toasts::<u8>::default();
        for i in 0..3 {
            q.push(ToastKind::Error, format!("Falha {i}"), None, t0);
        }
        q.push(ToastKind::Success, "Alteração desfeita", None, t0);
        let v = q.visible();
        assert_eq!(v.len(), MAX_VISIBLE);
        assert!(v.iter().any(|t| t.text == "Alteração desfeita"));
        assert!(!v.iter().any(|t| t.text == "Falha 0"));
    }

    #[test]
    fn newcomer_progress_does_not_evict_oldest_error_when_success_exists() {
        let t0 = Instant::now();
        let mut q = Toasts::<u8>::default();
        q.push(ToastKind::Error, "Falha a", None, t0);
        q.push(ToastKind::Success, "ok velho", None, t0);
        q.push(ToastKind::Error, "Falha b", None, t0);
        q.push(ToastKind::Progress, "andando", None, t0);
        let v = q.visible();
        assert_eq!(v.len(), MAX_VISIBLE);
        assert!(v.iter().any(|t| t.text == "Falha a"));
        assert!(!v.iter().any(|t| t.text == "ok velho"));
        assert!(v.iter().any(|t| t.text == "andando"));
    }

    #[test]
    fn legacy_data_rs_failures_are_errors() {
        for s in [
            "Cancelado (Reiniciar serviço): autenticação não concluída.",
            "apply-greeter retornou código 126 (autenticação cancelada ou erro).",
            "kitty retornou código 1.",
            "Nenhum terminal encontrado (cosmic-term, kitty, alacritty, foot, xterm).",
            "Configuração salva em /x, mas apply-greeter.sh não foi encontrado.",
            "A operação falhou.",
        ] {
            assert!(matches!(kind_from_legacy(s), ToastKind::Error), "{s}");
        }
        assert!(matches!(kind_from_legacy("Configuração salva."), ToastKind::Info));
    }

    #[test]
    fn legacy_prefixes() {
        assert!(matches!(kind_from_legacy("✗ deu ruim"), ToastKind::Error));
        assert!(matches!(kind_from_legacy("Falha ao salvar perfis"), ToastKind::Error));
        assert!(matches!(kind_from_legacy("✓ pronto"), ToastKind::Success));
        assert!(matches!(kind_from_legacy("Aplicando plano"), ToastKind::Info));
    }

    #[test]
    fn recent_keeps_last_three_and_undo_once() {
        let mut r = Recent::<u8>::default();
        let now = Instant::now();
        for i in 0..4u8 {
            r.record(RecentChange { label: format!("c{i}"), section: Section::Power, at: now, undo: Some(i) });
        }
        assert_eq!(r.items().len(), 3);
        assert_eq!(r.items()[0].label, "c3");
        assert_eq!(r.take_undo(0), Some(3));
        assert_eq!(r.take_undo(0), None);
    }
}
