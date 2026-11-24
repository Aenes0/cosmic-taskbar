use super::*;

fn make_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cosmic_greeter_style_test_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parse_all_fields_and_comments() {
    let toml = r##"
        # Full config
        style = "android" # inline comment
        wallpaper = "/tmp/wall.png"
        wallpaper_mode = "rotate_on_lock"
        wallpaper_dir = "/tmp/walls"
        wallpaper_shuffle = false
        wallpaper_interval_mins = 45
        blur_radius = 24
        dim_opacity = 0.35
        theme = "light"
        font_family = "mono"
        font_weight = "bold"
        clock_size = "huge"
        clock_24h = false
        show_seconds = true
        show_date = false
        show_avatar = false
        show_username = false
        card_opacity = 0.65
        card_radius = 22.0
        accent_color = "#ff5500"
    "##;
    let cfg = StyleConfig::parse(toml);
    assert_eq!(cfg.style, "android");
    assert_eq!(cfg.wallpaper, "/tmp/wall.png");
    assert_eq!(cfg.wallpaper_mode, "rotate_on_lock");
    assert_eq!(cfg.wallpaper_dir, "/tmp/walls");
    assert!(!cfg.wallpaper_shuffle);
    assert_eq!(cfg.wallpaper_interval_mins, 45);
    assert_eq!(cfg.blur_radius, 24);
    assert!((cfg.dim_opacity - 0.35).abs() < 1e-6);
    assert_eq!(cfg.theme, "light");
    assert_eq!(cfg.font_family, "mono");
    assert_eq!(cfg.font_weight, "bold");
    assert_eq!(cfg.clock_size, "huge");
    assert!(!cfg.clock_24h);
    assert!(cfg.show_seconds);
    assert!(!cfg.show_date);
    assert!(!cfg.show_avatar);
    assert!(!cfg.show_username);
    assert!((cfg.card_opacity - 0.65).abs() < 1e-6);
    assert!((cfg.card_radius - 22.0).abs() < 1e-6);
    assert_eq!(cfg.accent_color, "#ff5500");
    assert!(cfg.accent_color_rgba().is_some());
}

#[test]
fn parse_clamps_and_rejects_invalid_enums() {
    let toml = r#"
        style = "invalid"
        wallpaper_mode = "bogus"
        wallpaper_interval_mins = 0
        blur_radius = 999
        dim_opacity = 2.5
        card_opacity = -1.0
        card_radius = 100.0
        theme = "neon"
        font_family = "comic"
        font_weight = "black"
        clock_size = "tiny"
    "#;
    let cfg = StyleConfig::parse(toml);
    assert_eq!(cfg.style, "win11");
    assert_eq!(cfg.wallpaper_mode, "fixed");
    assert_eq!(cfg.wallpaper_interval_mins, 1);
    assert_eq!(cfg.blur_radius, 60);
    assert_eq!(cfg.dim_opacity, 0.85);
    assert_eq!(cfg.card_opacity, 0.0);
    assert_eq!(cfg.card_radius, 32.0);
    assert_eq!(cfg.theme, "dark");
    assert_eq!(cfg.font_family, "system");
    assert_eq!(cfg.font_weight, "normal");
    assert_eq!(cfg.clock_size, "medium");
}

#[test]
fn pick_wallpaper_fixed_and_rotate_modes() {
    let dir = make_temp_dir("pick");
    let sub = dir.join("sub");
    let deep = sub.join("deep");
    fs::create_dir_all(&deep).unwrap();

    let a = dir.join("a.png");
    let b = dir.join("b.JPG");
    let c = sub.join("c.webp");
    let ignored_txt = dir.join("notes.txt");
    let ignored_deep = deep.join("d.png");
    for p in [&a, &b, &c, &ignored_txt, &ignored_deep] {
        fs::write(p, b"img").unwrap();
    }

    // Fixed mode
    let mut cfg = StyleConfig::default();
    cfg.wallpaper_mode = "fixed".into();
    cfg.wallpaper = a.to_string_lossy().into_owned();
    assert_eq!(cfg.pick_wallpaper_at(0, 0), Some(a.clone()));
    assert_eq!(cfg.pick_wallpaper_at(5, 9999), Some(a.clone()));

    // Rotate on lock (sequential)
    cfg.wallpaper_mode = "rotate_on_lock".into();
    cfg.wallpaper_dir = dir.to_string_lossy().into_owned();
    cfg.wallpaper_shuffle = false;
    assert_eq!(cfg.pick_wallpaper_at(0, 0), Some(a.clone()));
    assert_eq!(cfg.pick_wallpaper_at(1, 0), Some(b.clone()));
    assert_eq!(cfg.pick_wallpaper_at(2, 0), Some(c.clone()));
    assert_eq!(cfg.pick_wallpaper_at(3, 0), Some(a.clone()));

    // Rotate on lock (deterministic shuffle covers all 3)
    cfg.wallpaper_shuffle = true;
    let picks: Vec<PathBuf> = (0..3)
        .map(|i| cfg.pick_wallpaper_at(i, 0).unwrap())
        .collect();
    assert_eq!(picks.len(), 3);
    assert!(picks.contains(&a) && picks.contains(&b) && picks.contains(&c));

    // Rotate interval (every 10 minutes = 600s)
    cfg.wallpaper_mode = "rotate_interval".into();
    cfg.wallpaper_interval_mins = 10;
    cfg.wallpaper_shuffle = false;
    assert_eq!(cfg.pick_wallpaper_at(0, 0), Some(a.clone()));
    assert_eq!(cfg.pick_wallpaper_at(0, 599), Some(a.clone()));
    assert_eq!(cfg.pick_wallpaper_at(0, 600), Some(b.clone()));
    assert_eq!(cfg.pick_wallpaper_at(0, 1200), Some(c.clone()));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn style_aliases_map_to_new_names() {
    assert_eq!(StyleConfig::parse("style = \"editorial\"").style, "ios");
    assert_eq!(StyleConfig::parse("style = \"sidebar\"").style, "ipados");
    assert_eq!(StyleConfig::parse("style = \"IPADOS\"").style, "ipados");
    assert_eq!(StyleConfig::parse("style = \"vista\"").style, "win11");
}

#[test]
fn parses_default_user_and_session() {
    let c = StyleConfig::parse("default_user = \"maria\"\nsession = \"COSMIC\"");
    assert_eq!((c.default_user.as_str(), c.session.as_str()), ("maria", "COSMIC"));
    assert_eq!(StyleConfig::default().session, "");
}

#[test]
fn rotation_slot_only_for_interval() {
    let mut c = StyleConfig::default();
    c.wallpaper_interval_mins = 10;
    assert_eq!(c.rotation_slot(1200), None);
    c.wallpaper_mode = "rotate_interval".into();
    assert_eq!(c.rotation_slot(599), Some(0));
    assert_eq!(c.rotation_slot(600), Some(1));
}

#[test]
fn login_rotate_on_lock_picks_by_time() {
    let dir = make_temp_dir("login_time");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    let c = dir.join("c.png");
    for p in [&a, &b, &c] {
        fs::write(p, b"img").unwrap();
    }
    let mut cfg = StyleConfig::default();
    cfg.wallpaper_mode = "rotate_on_lock".into();
    cfg.wallpaper_dir = dir.to_string_lossy().into_owned();
    cfg.wallpaper_shuffle = false;
    cfg.wallpaper_interval_mins = 10;
    assert_eq!(cfg.pick_wallpaper_login_at(0), Some(a.clone()));
    assert_eq!(cfg.pick_wallpaper_login_at(600), Some(b.clone()));
    assert_eq!(cfg.pick_wallpaper_login_at(1200), Some(c.clone()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rotation_due_only_on_slot_change() {
    assert!(rotation_due(Some(1), Some(2)));
    assert!(!rotation_due(Some(2), Some(2)));
    assert!(!rotation_due(None, None));
    assert!(!rotation_due(Some(1), None));
    assert!(!wants_keyboard_menu(1));
    assert!(wants_keyboard_menu(2));
}

#[test]
fn second_tick_never_on_android() {
    let mut c = StyleConfig::default();
    c.show_seconds = true;
    assert!(c.needs_second_tick());
    c.style = "android".into();
    assert!(!c.needs_second_tick());
}

#[test]
fn override_path_only_when_nested() {
    let p = candidate_config_paths_from(
        true,
        Some(OsStr::new("/tmp/x.toml")),
        None,
        Some("/h".into()),
    );
    assert_eq!(p[0], PathBuf::from("/tmp/x.toml"));
    let q = candidate_config_paths_from(
        false,
        Some(OsStr::new("/tmp/x.toml")),
        None,
        Some("/h".into()),
    );
    assert!(!q.contains(&PathBuf::from("/tmp/x.toml")));
}

#[test]
fn hex_color_parsing() {
    assert_eq!(parse_hex_color("#ffffff"), Some(Color::WHITE));
    assert_eq!(parse_hex_color("000000"), Some(Color::BLACK));
    assert_eq!(parse_hex_color(""), None);
    assert_eq!(parse_hex_color("#12345"), None);
    assert_eq!(parse_hex_color("#gggggg"), None);
}
