// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

const MONITOR: (u32, u32) = (64, 36);

fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .expect("codificar PNG em memória");
    out
}

/// 64×36: metade esquerda branca, direita preta.
fn sample_png() -> Vec<u8> {
    let mut img = RgbaImage::new(64, 36);
    for (x, _, px) in img.enumerate_pixels_mut() {
        *px = if x < 32 { Rgba([255, 255, 255, 255]) } else { Rgba([0, 0, 0, 255]) };
    }
    encode_png(&img)
}

fn tag_for(radius: u32, monitor: (u32, u32)) -> Tag {
    let handle = widget::image::Handle::from_bytes(vec![1u8, 2, 3]);
    Tag { image: handle.id(), radius, monitor }
}

/// Diretório temporário único por teste; o chamador apaga ao fim.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("aenes-backdrop-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("criar diretório temporário");
    dir
}

#[test]
fn reduced_size_caps_8k_and_ultrawide() {
    assert_eq!(reduced_size((7680, 4320)), (1920, 1080));
    assert_eq!(reduced_size((5120, 1440)), (1280, 360));
    assert_eq!(reduced_size((1, 1)), (1, 1));
}

#[test]
fn key_differs_per_monitor_size() {
    let bytes = sample_png();
    let a = key_for(&bytes, 8, (1920, 1080));
    let b = key_for(&bytes, 8, (5120, 1440));
    assert_ne!(a, b);
    assert_eq!(a, key_for(&bytes, 8, (1920, 1080)));
    assert_ne!(key_for(&bytes, 0, (1920, 1080)), key_for(&bytes, 20, (1920, 1080)));
}

#[test]
fn fnv_is_stable() {
    assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
    assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
}

#[test]
fn analysis_sees_left_bright_right_dark() {
    let p = prepare(&sample_png(), tag_for(0, MONITOR), None).expect("prepare");
    let left = p.analysis.region_luminance(Region { x0: 0.0, y0: 0.0, x1: 0.5, y1: 1.0 });
    let right = p.analysis.region_luminance(Region { x0: 0.5, y0: 0.0, x1: 1.0, y1: 1.0 });
    assert!(left > 0.8, "esquerda deveria ser clara, veio {left}");
    assert!(right < 0.05, "direita deveria ser escura, veio {right}");
}

#[test]
fn cache_hit_skips_decoding() {
    let dir = temp_dir("hit");
    let bytes = sample_png();
    let tag = tag_for(8, MONITOR);
    let first = prepare(&bytes, tag, Some(&dir)).expect("primeira preparação");
    let file = dir.join(cache_file_name(&first.key));
    assert!(file.exists(), "a primeira preparação deveria gravar o cache");

    let red = RgbaImage::from_pixel(first.key.width, first.key.height, Rgba([255, 0, 0, 255]));
    std::fs::write(&file, encode_png(&red)).expect("substituir o cache");

    let second = prepare(&bytes, tag, Some(&dir)).expect("segunda preparação");
    let _ = std::fs::remove_dir_all(&dir);
    let expected = [1.0f32, 0.0, 0.0];
    for (got, want) in second.analysis.average.iter().zip(expected) {
        assert!((got - want).abs() < 0.02, "média {:?} não veio do cache", second.analysis.average);
    }
}

#[test]
fn truncated_cache_is_recomputed() {
    let dir = temp_dir("trunc");
    let bytes = sample_png();
    let tag = tag_for(8, MONITOR);
    let file = dir.join(cache_file_name(&key_for(&bytes, 8, MONITOR)));
    std::fs::write(&file, [0u8; 10]).expect("gravar cache truncado");

    prepare(&bytes, tag, Some(&dir)).expect("deveria recalcular");
    let stored = std::fs::read(&file).expect("ler o cache regravado");
    let decoded = image::load_from_memory(&stored);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(decoded.is_ok(), "o cache deveria ter sido sobrescrito com PNG válido");
}

#[test]
fn prepare_without_writable_cache_still_ok() {
    let result = prepare(
        &sample_png(),
        tag_for(8, MONITOR),
        Some(Path::new("/proc/aenes-nao-existe")),
    );
    assert!(result.is_ok());
}

#[test]
fn prepare_rejects_garbage_without_panic() {
    assert!(prepare(b"nao e imagem", tag_for(8, MONITOR), None).is_err());
}

#[test]
fn radius_zero_has_no_blurred_handle() {
    let p = prepare(&sample_png(), tag_for(0, MONITOR), None).expect("prepare");
    assert!(p.blurred.is_none());
    let sum: f32 = p.analysis.luminance.iter().flatten().sum();
    assert!(sum > 0.0, "a análise deveria estar presente");
}

#[test]
fn surface_tag_tracks_inputs() {
    let h = widget::image::Handle::from_bytes(vec![9u8]);
    assert!(surface_tag(&h, 8, None).is_none());
    let base = surface_tag(&h, 8, Some((1920, 1080))).expect("tag");
    assert_eq!(Some(base), surface_tag(&h, 8, Some((1920, 1080))));
    assert_ne!(Some(base), surface_tag(&h, 16, Some((1920, 1080))));
    assert_ne!(Some(base), surface_tag(&h, 8, Some((5120, 1440))));
}

#[test]
fn view_handle_falls_back_to_original() {
    let p = prepare(&sample_png(), tag_for(8, MONITOR), None).expect("prepare");
    let orig = widget::image::Handle::from_bytes(vec![7u8]);
    let blurred = p.blurred.as_ref().expect("raio 8 deve desfocar");

    assert!(std::ptr::eq(pick_view_handle(0, Some(&p), p.tag, &orig), &orig));
    let other = Tag { radius: 99, ..p.tag };
    assert!(std::ptr::eq(pick_view_handle(8, Some(&p), other, &orig), &orig));
    assert!(std::ptr::eq(pick_view_handle(8, None, p.tag, &orig), &orig));
    assert!(std::ptr::eq(pick_view_handle(8, Some(&p), p.tag, &orig), blurred));
}

#[test]
fn set_discards_stale_result() {
    let bytes = sample_png();
    let tag_a = tag_for(8, (64, 36));
    let tag_b = tag_for(8, (128, 72));
    let prepared_a = prepare(&bytes, tag_a, None).expect("a");
    let prepared_b = prepare(&bytes, tag_b, None).expect("b");

    let mut set: BackdropSet<u32> = BackdropSet::default();
    assert!(set.wants(1, tag_a));
    set.mark_pending(1, tag_a);
    assert!(!set.wants(1, tag_a));
    set.mark_pending(1, tag_b);
    assert!(!set.accept(1, prepared_a));
    assert!(set.accept(1, prepared_b));
    assert!(!set.wants(1, tag_b));
    assert!(set.get(1).is_some());
    set.forget(1);
    assert!(set.get(1).is_none());
}

fn touch(path: &Path, age_secs: u64) {
    let when = std::time::SystemTime::now() - std::time::Duration::from_secs(age_secs);
    std::fs::File::options()
        .write(true)
        .open(path)
        .and_then(|f| f.set_modified(when))
        .expect("ajustar mtime");
}

fn cache_name(n: u64) -> String {
    format!("{n:016x}-r8-16x9.png")
}

#[test]
fn prune_keeps_newest_cache_files_only() {
    let dir = temp_dir("prune");
    // 30 arquivos do cache; o de índice i tem i segundos de idade (0 = mais novo).
    for i in 0..30u64 {
        let path = dir.join(cache_name(i));
        std::fs::write(&path, b"x").expect("criar arquivo do cache");
        touch(&path, i + 10);
    }
    let foreign = dir.join("notas.txt");
    std::fs::write(&foreign, b"meu").expect("criar arquivo alheio");
    touch(&foreign, 100_000);
    let foreign_png = dir.join("foto.png");
    std::fs::write(&foreign_png, b"meu").expect("criar png alheio");
    touch(&foreign_png, 100_000);

    prune_cache(&dir, CACHE_KEEP, None);

    let kept: Vec<u64> = (0..30).filter(|i| dir.join(cache_name(*i)).exists()).collect();
    let foreign_alive = foreign.exists() && foreign_png.exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(kept, (0..24).collect::<Vec<u64>>(), "sobram os 24 mais novos");
    assert!(foreign_alive, "arquivo fora do padrão do cache nunca é apagado");
}

#[test]
fn prune_never_removes_the_protected_file() {
    let dir = temp_dir("prune-protect");
    for i in 0..5u64 {
        let path = dir.join(cache_name(i));
        std::fs::write(&path, b"x").expect("criar arquivo do cache");
        touch(&path, 10); // mesmo mtime em todos
    }
    let protected = cache_name(4);
    prune_cache(&dir, 2, Some(std::ffi::OsStr::new(&protected)));
    let alive = dir.join(&protected).exists();
    let left = std::fs::read_dir(&dir).expect("ler").count();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(alive, "o arquivo recém-gravado é preservado");
    assert_eq!(left, 2);
}

#[test]
fn cache_name_pattern_is_strict() {
    assert!(is_cache_file_name(&cache_name(1)));
    assert!(is_cache_file_name("00000000000000ff-r0-1920x1080.png"));
    assert!(!is_cache_file_name("foto.png"));
    assert!(!is_cache_file_name(&format!("{}.tmp", cache_name(1))));
    assert!(!is_cache_file_name("00000000000000ff-r8-16x9.jpg"));
    assert!(!is_cache_file_name("00000000000000f-r8-16x9.png"));
    assert!(!is_cache_file_name("00000000000000fg-r8-16x9.png"));
}

#[test]
fn prepare_prunes_the_cache_directory() {
    let dir = temp_dir("prune-prepare");
    for i in 0..30u64 {
        let path = dir.join(cache_name(i));
        std::fs::write(&path, b"x").expect("criar arquivo do cache");
        touch(&path, i + 10);
    }
    let bytes = sample_png();
    let p = prepare(&bytes, tag_for(8, MONITOR), Some(&dir)).expect("prepare");
    let fresh = dir.join(cache_file_name(&p.key)).exists();
    let count = std::fs::read_dir(&dir).expect("ler").count();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(fresh, "o cache novo fica");
    assert_eq!(count, CACHE_KEEP);
}

fn prepared_with(tag: Tag) -> Prepared {
    prepare(&sample_png(), tag, None).expect("prepare")
}

#[test]
fn view_keeps_previous_blur_while_new_tag_is_pending() {
    let tag_a = tag_for(8, (64, 36));
    let tag_b = tag_for(8, (128, 72));
    let orig = widget::image::Handle::from_bytes(vec![7u8]);
    let mut set: BackdropSet<u32> = BackdropSet::default();

    // Primeira pintura: nada pronto, vai a original.
    assert!(std::ptr::eq(set.view_handle(1, 8, tag_a, &orig), &orig));
    set.mark_pending(1, tag_a);
    assert!(std::ptr::eq(set.view_handle(1, 8, tag_a, &orig), &orig));
    assert!(set.accept(1, prepared_with(tag_a)));
    let blurred_a = set.get(1).and_then(|p| p.blurred.clone()).expect("a desfocado");
    assert_eq!(set.view_handle(1, 8, tag_a, &orig).id(), blurred_a.id());

    // Rotação: b pendente, a segue na tela (desfocado), nunca a original nítida.
    set.mark_pending(1, tag_b);
    assert_eq!(set.view_handle(1, 8, tag_b, &orig).id(), blurred_a.id());

    // b aceito: passa ao de b.
    assert!(set.accept(1, prepared_with(tag_b)));
    let blurred_b = set.get(1).and_then(|p| p.blurred.clone()).expect("b desfocado");
    assert_eq!(set.view_handle(1, 8, tag_b, &orig).id(), blurred_b.id());
    assert_ne!(blurred_a.id(), blurred_b.id());

    // Desfoque desligado: sempre a original.
    assert!(std::ptr::eq(set.view_handle(1, 0, tag_b, &orig), &orig));
}

#[test]
fn failed_job_drops_the_previous_blur() {
    let tag_a = tag_for(8, (64, 36));
    let tag_b = tag_for(8, (128, 72));
    let orig = widget::image::Handle::from_bytes(vec![7u8]);
    let mut set: BackdropSet<u32> = BackdropSet::default();
    set.mark_pending(1, tag_a);
    assert!(set.accept(1, prepared_with(tag_a)));
    set.mark_pending(1, tag_b);
    set.drop_ready(1);
    assert!(std::ptr::eq(set.view_handle(1, 8, tag_b, &orig), &orig));
    assert!(!set.wants(1, tag_b), "o pedido segue marcado: sem nova tentativa");
}
