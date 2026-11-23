// SPDX-License-Identifier: GPL-3.0-only

//! Cached `.desktop` icon lookup for open windows, kept out of `view()`.

use std::collections::HashMap;

use cosmic::{
    desktop::{IconSourceExt, fde},
    widget::icon,
};

struct DesktopMeta {
    id_lower: String,
    wm_class_lower: Option<String>,
    icon_name: String,
}

/// Resolves Wayland `app_id`s to cached `icon::Handle`s via `.desktop` files.
pub struct IconResolver {
    entries: Vec<DesktopMeta>,
    cache: HashMap<String, icon::Handle>,
    fallback: icon::Handle,
    loaded: bool,
}

impl Default for IconResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl IconResolver {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cache: HashMap::new(),
            fallback: named_fallback(&["application-default", "application-x-executable"]),
            loaded: false,
        }
    }

    /// Loads `.desktop` metadata once (or refreshes when asked).
    pub fn reload(&mut self) {
        let mut entries = Vec::new();
        for path in fde::Iter::new(fde::default_paths()) {
            let Ok(de) = fde::DesktopEntry::from_path::<String>(path, None) else {
                continue;
            };
            let icon_name = de.icon().unwrap_or(&de.appid).to_owned();
            entries.push(DesktopMeta {
                id_lower: de.appid.to_lowercase(),
                wm_class_lower: de.startup_wm_class().map(str::to_lowercase),
                icon_name,
            });
        }
        self.entries = entries;
        self.cache.clear();
        self.loaded = true;
    }

    /// Ensures icons are cached for every `app_id` in `app_ids` (call in `update`).
    pub fn ensure_all<'a>(&mut self, app_ids: impl IntoIterator<Item = &'a str>) {
        if !self.loaded {
            self.reload();
        }
        for app_id in app_ids {
            if app_id.is_empty() || self.cache.contains_key(app_id) {
                continue;
            }
            let handle = self.resolve_uncached(app_id);
            self.cache.insert(app_id.to_owned(), handle);
        }
    }

    /// Cheap lookup safe to call in `view()`.
    pub fn get(&self, app_id: &str) -> icon::Handle {
        self.cache
            .get(app_id)
            .cloned()
            .unwrap_or_else(|| self.fallback.clone())
    }

    fn resolve_uncached(&self, app_id: &str) -> icon::Handle {
        let wanted = app_id.to_lowercase();
        if let Some(entry) = self
            .entries
            .iter()
            .find(|e| e.id_lower == wanted)
            .or_else(|| {
                self.entries.iter().find(|e| {
                    e.wm_class_lower.as_deref() == Some(wanted.as_str())
                        || dotted_suffix(&e.id_lower, &wanted)
                })
            })
        {
            return fde::IconSource::from_unknown(&entry.icon_name).as_cosmic_icon();
        }
        named_fallback(&[app_id, "application-default", "application-x-executable"])
    }
}

fn named_fallback(names: &[&str]) -> icon::Handle {
    icon::from_name(names[0])
        .size(64)
        .fallback(Some(icon::IconFallback::Names(
            names[1..].iter().map(|n| (*n).to_owned().into()).collect(),
        )))
        .handle()
}

/// Whether `hay` ends with `.<wanted>`.
pub fn dotted_suffix(hay: &str, wanted: &str) -> bool {
    !wanted.is_empty()
        && hay.ends_with(wanted)
        && hay.len() > wanted.len()
        && hay.as_bytes().get(hay.len() - wanted.len() - 1) == Some(&b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_suffix_matches_reverse_dns_app_ids() {
        assert!(dotted_suffix("org.mozilla.firefox", "firefox"));
        assert!(dotted_suffix("com.brave.browser", "browser"));
        assert!(!dotted_suffix("firefox", "firefox"));
        assert!(!dotted_suffix("myfirefox", "firefox"));
        assert!(!dotted_suffix("org.mozilla.firefox", ""));
    }
}
