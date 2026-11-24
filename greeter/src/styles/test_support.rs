// SPDX-License-Identifier: GPL-3.0-only

//! Fixture de teste compartilhado: monta um [`ScreenInput`](super::ScreenInput) completo para os
//! testes de cada estilo. Só compilado em teste.

use cosmic::widget;

use super::{ChoiceMenu, FieldParts, Looks, PowerParts, ScreenInput, UserParts};
use crate::backdrop::Analysis;
use crate::login_ui::{ClockData, FieldFx, PowerFx, ScreenMetrics, StatusLine};
use crate::motion::Look;
use crate::style_config::StyleConfig;
use crate::sysinfo::{BatteryDevice, DeviceKind, NetworkInfo, NetworkKind, SystemInfo};

/// Mensagem de teste: nenhuma interação é exercitada, só a construção da view.
#[derive(Clone, Debug)]
pub(crate) enum TestMsg {
    Noop,
}

fn noop_action(_: crate::power::PowerAction) -> TestMsg {
    TestMsg::Noop
}

/// Config padrão com `style` e `theme` trocados.
pub(crate) fn cfg(style: &str, theme: &str) -> StyleConfig {
    StyleConfig {
        style: style.to_string(),
        theme: theme.to_string(),
        ..StyleConfig::default()
    }
}

/// Sistema de teste: um mouse a 87% e rede Wi-Fi "Casa".
pub(crate) fn system() -> SystemInfo {
    SystemInfo {
        batteries: vec![BatteryDevice {
            kind: DeviceKind::Mouse,
            name: "MX Master 3".into(),
            percent: 87,
            charging: false,
        }],
        network: Some(NetworkInfo {
            kind: NetworkKind::Wireless { strength: 80 },
            name: "Casa".into(),
        }),
    }
}

/// Inicializa o carregador de idiomas que `fl!` usa; chame no início de todo teste de view.
pub(crate) fn init_i18n() {
    crate::localize::localizer();
}

/// `ScreenInput` completo (energia, sessão, relógio, usuário); `with_field` liga o campo de senha.
pub(crate) fn input<'a>(
    cfg: &'a StyleConfig,
    system: &'a SystemInfo,
    backdrop: Option<&'a Analysis>,
    with_field: bool,
) -> ScreenInput<'a, TestMsg> {
    let look = Look::REST;
    let field = with_field.then(|| FieldParts {
        input: widget::text_input("Senha", ""),
        submit: Some(TestMsg::Noop),
        fx: FieldFx {
            look,
            hover: 0.0,
            focus: 0.0,
            submit_hover: 0.0,
            field_hovers: None,
            submit_hovers: None,
        },
    });
    ScreenInput {
        cfg,
        metrics: ScreenMetrics { avatar: 96.0, clock_top: 80.0, time_size: 96.0 },
        clock: ClockData {
            time: "10:42".into(),
            hour: "10".into(),
            minute: "42".into(),
            date: "terça-feira, 24 de novembro".into(),
            short_date: "ter., 24 de nov.".into(),
            look,
        },
        user: UserParts { icon: None, full_name: "Ana Teste" },
        field,
        status: StatusLine::Empty,
        looks: Looks { clock: look, avatar: look, name: look, field: look, status: look, power: look },
        power: Some(PowerParts {
            open: false,
            on_toggle: TestMsg::Noop,
            on_action: noop_action,
            fx: PowerFx { look, hover: 0.0, menu: look, hovers: None },
        }),
        session: Some(ChoiceMenu {
            open: false,
            on_toggle: TestMsg::Noop,
            label: "Opções de entrada".into(),
            items: vec![
                ("COSMIC".into(), true, TestMsg::Noop),
                ("GNOME".into(), false, TestMsg::Noop),
            ],
        }),
        keyboard: None,
        system,
        backdrop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_builds_dark_and_light() {
        init_i18n();
        let analysis = Analysis::uniform(0.9, [1.0, 1.0, 1.0]);
        let sys = system();
        for style in crate::styles::STYLE_NAMES {
            for theme in ["dark", "light"] {
                let config = cfg(style, theme);
                let _element = crate::styles::view(input(&config, &sys, Some(&analysis), true));
            }
        }
    }
}
