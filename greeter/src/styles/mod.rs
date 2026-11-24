// SPDX-License-Identifier: GPL-3.0-only

//! Os 5 estilos da tela de bloqueio e de login. Locker e greeter montam um [`ScreenInput`] com as
//! peças cruas (usuário, campo, energia, menus, dados do sistema, análise do fundo) e chamam
//! [`view`], que despacha para `styles/<estilo>.rs` por `cfg.style`.

use cosmic::Element;
use cosmic::widget;

use crate::login_ui::{self, ClockData};
use crate::motion::Look;
use crate::style_config::StyleConfig;

pub mod android;
pub mod ios;
pub mod ipados;
pub mod macos;
pub mod palette;
pub mod parts;
#[cfg(test)]
pub(crate) mod test_support;
pub mod win11;

/// Nomes canônicos aceitos em `cfg.style`, na ordem de [`Style::ALL`]. Nome desconhecido cai em
/// `win11`; sinônimos antigos (`editorial`, `sidebar`) são normalizados pelo parser da config.
pub const STYLE_NAMES: [&str; 5] = ["win11", "android", "macos", "ios", "ipados"];

/// Estilo resolvido a partir de `cfg.style`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Win11,
    Android,
    Macos,
    Ios,
    Ipados,
}

impl Style {
    /// Mesma ordem de [`STYLE_NAMES`].
    const ALL: [Style; 5] = [
        Style::Win11,
        Style::Android,
        Style::Macos,
        Style::Ios,
        Style::Ipados,
    ];

    /// O estilo de `name`; qualquer nome fora de [`STYLE_NAMES`] vira `win11`.
    fn from_name(name: &str) -> Style {
        STYLE_NAMES
            .iter()
            .position(|known| *known == name)
            .map_or(Style::Win11, |index| Style::ALL[index])
    }
}

/// Foto e nome completo do usuário.
pub struct UserParts<'a> {
    pub icon: Option<&'a widget::image::Handle>,
    pub full_name: &'a str,
}

/// Campo de senha (ou de usuário): input já com id, `on_input`, `on_submit`, `on_focus` e
/// `on_unfocus`; sem padding, largura nem estilo (o estilo decide).
pub struct FieldParts<'a, M> {
    pub input: widget::TextInput<'a, M>,
    pub submit: Option<M>,
    pub fx: login_ui::FieldFx<M>,
}

/// Botão de energia e o menu dele.
pub struct PowerParts<M> {
    pub open: bool,
    pub on_toggle: M,
    pub on_action: fn(crate::power::PowerAction) -> M,
    pub fx: login_ui::PowerFx<M>,
}

/// Menu de escolha (sessão, layout do teclado).
pub struct ChoiceMenu<M> {
    pub open: bool,
    pub on_toggle: M,
    pub label: String,
    /// (rótulo, marcado, mensagem)
    pub items: Vec<(String, bool, M)>,
}

/// Aparência animada (fade e deslocamento) de cada peça.
pub struct Looks {
    pub clock: Look,
    pub avatar: Look,
    pub name: Look,
    pub field: Look,
    pub status: Look,
    pub power: Look,
}

/// Tudo o que um estilo precisa para montar a tela.
pub struct ScreenInput<'a, M> {
    pub cfg: &'a StyleConfig,
    pub metrics: login_ui::ScreenMetrics,
    pub clock: ClockData,
    pub user: UserParts<'a>,
    pub field: Option<FieldParts<'a, M>>,
    pub status: login_ui::StatusLine<'a>,
    pub looks: Looks,
    pub power: Option<PowerParts<M>>,
    /// Só o greeter (login).
    pub session: Option<ChoiceMenu<M>>,
    /// Só com 2+ layouts ativos.
    pub keyboard: Option<ChoiceMenu<M>>,
    pub system: &'a crate::sysinfo::SystemInfo,
    pub backdrop: Option<&'a crate::backdrop::Analysis>,
}

/// Monta a tela no estilo `input.cfg.style`.
pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M> {
    match Style::from_name(&input.cfg.style) {
        Style::Win11 => win11::view(input),
        Style::Android => android::view(input),
        Style::Macos => macos::view(input),
        Style::Ios => ios::view(input),
        Style::Ipados => ipados::view(input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_names_are_canonical() {
        assert_eq!(STYLE_NAMES, ["win11", "android", "macos", "ios", "ipados"]);
    }

    #[test]
    fn every_canonical_name_resolves_to_its_own_style() {
        for (index, name) in STYLE_NAMES.iter().enumerate() {
            assert_eq!(Style::from_name(name), Style::ALL[index], "{name}");
        }
    }

    #[test]
    fn unknown_name_falls_back_to_win11() {
        assert_eq!(Style::from_name("nope"), Style::Win11);
        assert_eq!(Style::from_name(""), Style::Win11);
        // Os sinônimos antigos são normalizados pelo parser, não pelo despacho.
        assert_eq!(Style::from_name("editorial"), Style::Win11);
        assert_eq!(Style::from_name("sidebar"), Style::Win11);
    }
}
