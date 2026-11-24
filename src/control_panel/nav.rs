// SPDX-License-Identifier: GPL-3.0-only

//! Navegação por categorias do painel de controle (entrega painel-b).

use super::Section;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Home,
    System,
    Appearance,
    Tools,
    Protection,
    Maintenance,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Home,
        Category::System,
        Category::Appearance,
        Category::Tools,
        Category::Protection,
        Category::Maintenance,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Home => "Início",
            Category::System => "Sistema",
            Category::Appearance => "Aparência",
            Category::Tools => "Ferramentas",
            Category::Protection => "Proteção",
            Category::Maintenance => "Manutenção",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Category::Home => "go-home-symbolic",
            Category::System => "computer-symbolic",
            Category::Appearance => "applications-graphics-symbolic",
            Category::Tools => "applications-utilities-symbolic",
            Category::Protection => "security-high-symbolic",
            Category::Maintenance => "emblem-system-symbolic",
        }
    }

    pub fn sections(self) -> &'static [Section] {
        match self {
            Category::Home => &[Section::Home],
            Category::System => &[Section::Power, Section::Displays, Section::Network, Section::Camera],
            Category::Appearance => &[
                Section::Bar,
                Section::Central,
                Section::Monitor,
                Section::Notifications,
                Section::DevicesOsd,
                Section::Login,
            ],
            Category::Tools => &[Section::Capture],
            Category::Protection => &[Section::Security, Section::Backup],
            Category::Maintenance => &[Section::Optional, Section::System],
        }
    }
}

pub fn category_of(s: Section) -> Category {
    match s {
        Section::Home => Category::Home,
        Section::Power | Section::Displays | Section::Network | Section::Camera => Category::System,
        Section::Bar
        | Section::Central
        | Section::Monitor
        | Section::Notifications
        | Section::DevicesOsd
        | Section::Login => Category::Appearance,
        Section::Capture => Category::Tools,
        Section::Security | Section::Backup => Category::Protection,
        Section::Optional | Section::System => Category::Maintenance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_exactly_one_category() {
        for s in Section::ALL {
            assert_eq!(Category::ALL.iter().filter(|c| c.sections().contains(&s)).count(), 1, "{s:?}");
        }
    }

    #[test]
    fn from_arg_keeps_old_values_and_accepts_new() {
        for (a, s) in [
            ("power", Section::Power),
            ("energia", Section::Power),
            ("tray", Section::Bar),
            ("rede", Section::Network),
            ("monitores", Section::Displays),
            ("greeter", Section::Login),
            ("sistema", Section::System),
            ("capture", Section::Capture),
            ("captura", Section::Capture),
            ("print", Section::Capture),
            ("screenshot", Section::Capture),
            ("home", Section::Home),
            ("inicio", Section::Home),
            (" POWER ", Section::Power),
        ] {
            assert_eq!(Section::from_arg(a), Some(s), "{a}");
        }
        assert_eq!(Section::from_arg("xyz"), None);
    }

    #[test]
    fn capture_and_osd_categories() {
        assert_eq!(category_of(Section::Capture), Category::Tools);
        assert_eq!(category_of(Section::DevicesOsd), Category::Appearance);
        assert_eq!(Category::ALL[0], Category::Home);
    }
}
