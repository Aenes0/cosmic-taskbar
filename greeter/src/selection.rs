// SPDX-License-Identifier: GPL-3.0-only

//! Escolha do usuário e da sessão padrão do greeter.

/// Índice do usuário a selecionar em `users` (pares nome de login, uid).
///
/// Prioridade: `default_user` (não vazio e presente, por nome) → `last_user_uid` presente →
/// o primeiro. Lista vazia → `None`.
pub fn pick_user(
    default_user: &str,
    last_user_uid: Option<u32>,
    users: &[(String, u32)],
) -> Option<usize> {
    if users.is_empty() {
        return None;
    }
    if !default_user.is_empty() {
        if let Some(i) = users.iter().position(|(name, _)| name == default_user) {
            return Some(i);
        }
    }
    if let Some(uid) = last_user_uid {
        if let Some(i) = users.iter().position(|(_, u)| *u == uid) {
            return Some(i);
        }
    }
    Some(0)
}

/// Nome da sessão a selecionar entre `available`.
///
/// Prioridade: `configured` (não vazio e presente) → `last_session` presente → a primeira.
/// Lista vazia → `None`.
pub fn pick_session(
    configured: &str,
    last_session: Option<&str>,
    available: &[String],
) -> Option<String> {
    if !configured.is_empty() && available.iter().any(|s| s == configured) {
        return Some(configured.to_string());
    }
    if let Some(last) = last_session {
        if available.iter().any(|s| s == last) {
            return Some(last.to_string());
        }
    }
    available.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users() -> [(String, u32); 2] {
        [("ana".to_string(), 1000), ("bia".to_string(), 1001)]
    }

    fn sessions() -> Vec<String> {
        vec!["COSMIC".to_string(), "GNOME".to_string()]
    }

    #[test]
    fn pick_user_default_wins_over_last() {
        assert_eq!(pick_user("bia", Some(1000), &users()), Some(1));
    }

    #[test]
    fn pick_user_unknown_default_falls_to_last() {
        assert_eq!(pick_user("zeca", Some(1001), &users()), Some(1));
    }

    #[test]
    fn pick_user_empty_default_uses_last_then_first() {
        assert_eq!(pick_user("", Some(1001), &users()), Some(1));
        assert_eq!(pick_user("", Some(4242), &users()), Some(0));
        assert_eq!(pick_user("", None, &users()), Some(0));
    }

    #[test]
    fn pick_user_no_users() {
        assert_eq!(pick_user("ana", None, &[]), None);
    }

    #[test]
    fn pick_session_configured_wins() {
        assert_eq!(
            pick_session("COSMIC", Some("GNOME"), &sessions()),
            Some("COSMIC".to_string())
        );
    }

    #[test]
    fn pick_session_missing_configured_uses_last() {
        assert_eq!(
            pick_session("Plasma", Some("GNOME"), &sessions()),
            Some("GNOME".to_string())
        );
    }

    #[test]
    fn pick_session_missing_last_uses_first() {
        assert_eq!(
            pick_session("", Some("Hyprland"), &sessions()),
            Some("COSMIC".to_string())
        );
    }

    #[test]
    fn pick_session_empty_list_is_none() {
        assert_eq!(pick_session("COSMIC", None, &[]), None);
    }
}
