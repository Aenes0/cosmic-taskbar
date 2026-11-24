// SPDX-License-Identifier: GPL-3.0-only

//! Desfoque e análise do papel de parede, fora da thread de UI: redução, desfoque, cache em disco
//! (PNG por chave), luminância por região e o conjunto de resultados por superfície.


use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use cosmic::iced::core::Bytes;
use cosmic::iced::core::image::Id;
use cosmic::widget;

/// Lado da grade de luminância (`GRID` × `GRID` regiões).
pub const GRID: usize = 8;

/// Retângulo normalizado do fundo (`0..=1` nos dois eixos).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// Resultado da análise do fundo de um monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// `[linha][coluna]`, luminância relativa WCAG `0..=1`, já em "cover".
    pub luminance: [[f32; GRID]; GRID],
    /// Cor média, sRGB `0..=1`.
    pub average: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BackdropKey {
    pub hash: u64,
    pub radius: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tag {
    pub image: Id,
    pub radius: u32,
    pub monitor: (u32, u32),
}

#[derive(Clone, Debug)]
pub struct Prepared {
    pub tag: Tag,
    pub key: BackdropKey,
    pub blurred: Option<widget::image::Handle>,
    pub analysis: Analysis,
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
/// Cada lado do fundo guardado é o do monitor dividido por isto (4× menos pixels por eixo).
const REDUCTION: u32 = 4;
const BOOT_CACHE_DIR: &str = "/var/lib/aenes-greeter/cache";
const CACHE_SUBDIR: &str = "aenes-greeter";
/// Quantos PNGs do cache ficam na pasta depois de cada gravação (os mais recentes).
const CACHE_KEEP: usize = 24;

/// FNV-1a de 64 bits (sem dependência nova; só identifica o arquivo do fundo, não é criptográfico).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME))
}

/// Tamanho do fundo guardado para um monitor: cada lado ÷ 4, arredondado para cima, mínimo 1.
pub fn reduced_size(monitor: (u32, u32)) -> (u32, u32) {
    let side = |v: u32| v.div_ceil(REDUCTION).max(1);
    (side(monitor.0), side(monitor.1))
}

pub fn key_for(bytes: &[u8], radius: u32, monitor: (u32, u32)) -> BackdropKey {
    let (width, height) = reduced_size(monitor);
    BackdropKey { hash: fnv1a64(bytes), radius, width, height }
}

pub fn cache_file_name(key: &BackdropKey) -> String {
    format!("{:016x}-r{}-{}x{}.png", key.hash, key.radius, key.width, key.height)
}

/// Pasta do cache: no boot, `/var/lib/aenes-greeter/cache` se existir; senão (e fora do boot)
/// `$XDG_CACHE_HOME` ou `~/.cache` + `aenes-greeter`, criada com 0700. `None` se nada serve.
pub fn cache_dir(boot: bool) -> Option<PathBuf> {
    if boot {
        let dir = PathBuf::from(BOOT_CACHE_DIR);
        if dir.is_dir() {
            return Some(dir);
        }
    }
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    let dir = base.join(CACHE_SUBDIR);
    match private_dir(&dir) {
        Ok(()) => Some(dir),
        Err(err) => {
            tracing::debug!("cache do fundo indisponível em {}: {err}", dir.display());
            None
        }
    }
}

fn private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
}

/// Bytes codificados do handle, só quando já estão na memória (clone barato). `Path` não é
/// lido aqui (seria I/O na thread de UI) e `Rgba` não é imagem codificada.
pub fn handle_bytes(handle: &widget::image::Handle) -> Option<Bytes> {
    match handle {
        widget::image::Handle::Bytes(_, bytes) => Some(bytes.clone()),
        _ => None,
    }
}

/// Luminância relativa WCAG (sRGB → linear) de cada canal, para os 256 valores possíveis.
fn linear_table() -> [f32; 256] {
    std::array::from_fn(|i| {
        let c = i as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    })
}

/// Grade `GRID`×`GRID` de luminância relativa média e cor média (sRGB) da imagem.
pub fn analyze(img: &image::RgbaImage) -> Analysis {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Analysis::uniform(0.0, [0.0; 3]);
    }
    let table = linear_table();
    let mut cell_sum = [[0.0f64; GRID]; GRID];
    let mut cell_count = [[0u32; GRID]; GRID];
    let mut channel_sum = [0u64; 3];
    for (x, y, px) in img.enumerate_pixels() {
        let [r, g, b, _] = px.0;
        let lum = 0.2126 * table[r as usize] + 0.7152 * table[g as usize] + 0.0722 * table[b as usize];
        let col = (x as usize * GRID / w as usize).min(GRID - 1);
        let row = (y as usize * GRID / h as usize).min(GRID - 1);
        cell_sum[row][col] += f64::from(lum);
        cell_count[row][col] += 1;
        channel_sum[0] += u64::from(r);
        channel_sum[1] += u64::from(g);
        channel_sum[2] += u64::from(b);
    }
    // Imagem menor que a grade deixa células sem pixel: elas herdam o pixel que as cobre.
    let mut luminance = [[0.0f32; GRID]; GRID];
    for row in 0..GRID {
        for col in 0..GRID {
            luminance[row][col] = if cell_count[row][col] > 0 {
                (cell_sum[row][col] / f64::from(cell_count[row][col])) as f32
            } else {
                let x = (col * w as usize / GRID).min(w as usize - 1) as u32;
                let y = (row * h as usize / GRID).min(h as usize - 1) as u32;
                let [r, g, b, _] = img.get_pixel(x, y).0;
                0.2126 * table[r as usize] + 0.7152 * table[g as usize] + 0.0722 * table[b as usize]
            };
        }
    }
    let total = f64::from(w) * f64::from(h) * 255.0;
    let average = [0, 1, 2].map(|c| (channel_sum[c] as f64 / total) as f32);
    Analysis { luminance, average }
}

/// Lê o cache se existir, decodificar e tiver as dimensões da chave; qualquer outra coisa é miss.
fn read_cache(path: &Path, key: &BackdropKey) -> Option<image::RgbaImage> {
    let bytes = std::fs::read(path).ok()?;
    let img = match image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
        Ok(img) => img.to_rgba8(),
        Err(err) => {
            tracing::debug!("cache do fundo inválido em {}: {err}", path.display());
            return None;
        }
    };
    (img.dimensions() == (key.width, key.height)).then_some(img)
}

/// `true` se `name` segue o padrão de `cache_file_name`: `<16 hex>-r<raio>-<larg>x<alt>.png`.
fn is_cache_file_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".png") else {
        return false;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let mut parts = stem.splitn(3, '-');
    let (Some(hash), Some(radius), Some(size)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let hash_ok = hash.len() == 16 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let radius_ok = radius.strip_prefix('r').is_some_and(digits);
    let size_ok = size.split_once('x').is_some_and(|(w, h)| digits(w) && digits(h));
    hash_ok && radius_ok && size_ok
}

/// Mantém só os `keep` PNGs do cache mais recentes (por mtime) em `dir`; nunca apaga arquivo fora do
/// padrão do cache, nem `protect`. Erro de I/O só vira `debug`.
fn prune_cache(dir: &Path, keep: usize, protect: Option<&std::ffi::OsStr>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::debug!("não foi possível listar o cache do fundo {}: {err}", dir.display());
            return;
        }
    };
    let mut files: Vec<(bool, std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_str().is_some_and(is_cache_file_name))
        .filter_map(|entry| {
            let meta = entry.metadata().ok().filter(|m| m.is_file())?;
            let mtime = meta.modified().ok()?;
            let is_protected = protect.is_some_and(|p| entry.file_name() == p);
            Some((is_protected, mtime, entry.path()))
        })
        .collect();
    // Protegido primeiro, depois o mais novo; nome desempata mtimes iguais.
    files.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then_with(|| a.2.cmp(&b.2)));
    for (_, _, path) in files.into_iter().skip(keep) {
        if let Err(err) = std::fs::remove_file(&path) {
            tracing::debug!("não foi possível podar o cache do fundo {}: {err}", path.display());
        }
    }
}

/// Grava o PNG em arquivo temporário único e renomeia; falha só vira `debug` (segue sem cache).
fn write_cache(path: &Path, img: &image::RgbaImage) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let mut encoded = Vec::new();
    if let Err(err) = img.write_to(&mut std::io::Cursor::new(&mut encoded), image::ImageFormat::Png) {
        tracing::debug!("não foi possível codificar o cache do fundo: {err}");
        return;
    }
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(format!(".{}-{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    let tmp = PathBuf::from(tmp_name);
    let result = std::fs::write(&tmp, &encoded).and_then(|()| std::fs::rename(&tmp, path));
    if let Err(err) = result {
        tracing::debug!("não foi possível gravar o cache do fundo {}: {err}", path.display());
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if let (Some(dir), Some(name)) = (path.parent(), path.file_name()) {
        prune_cache(dir, CACHE_KEEP, Some(name));
    }
}

/// Reduz, desfoca (se `radius > 0`), grava o cache e analisa. Bloqueante: nunca na thread de UI.
/// Bytes corrompidos devolvem `Err`; cache ausente, ilegível ou truncado só faz recalcular.
pub fn prepare(bytes: &[u8], tag: Tag, cache: Option<&Path>) -> Result<Prepared, String> {
    let key = key_for(bytes, tag.radius, tag.monitor);
    let cache_path = cache.map(|dir| dir.join(cache_file_name(&key)));

    let img = match cache_path.as_deref().and_then(|path| read_cache(path, &key)) {
        Some(cached) => cached,
        None => {
            let decoded = image::load_from_memory(bytes)
                .map_err(|err| format!("não foi possível decodificar o papel de parede: {err}"))?;
            let reduced = decoded
                .resize_to_fill(key.width, key.height, image::imageops::FilterType::Triangle)
                .to_rgba8();
            let finished = if tag.radius > 0 {
                image::imageops::fast_blur(&reduced, tag.radius as f32 / 4.0)
            } else {
                reduced
            };
            if let Some(path) = cache_path.as_deref() {
                write_cache(path, &finished);
            }
            finished
        }
    };

    let analysis = analyze(&img);
    let blurred = (tag.radius > 0).then(|| {
        let (w, h) = img.dimensions();
        widget::image::Handle::from_rgba(w, h, img.into_raw())
    });
    Ok(Prepared { tag, key, blurred, analysis })
}

/// `prepare` num thread de bloqueio do tokio.
pub async fn prepare_async(
    bytes: Bytes,
    tag: Tag,
    cache: Option<PathBuf>,
) -> Result<Prepared, String> {
    tokio::task::spawn_blocking(move || prepare(&bytes, tag, cache.as_deref()))
        .await
        .map_err(|err| format!("tarefa do fundo falhou: {err}"))?
}

impl Analysis {
    /// Luminância média da região, ponderada pela área que cada célula da grade cobre.
    pub fn region_luminance(&self, r: Region) -> f32 {
        let (x0, x1) = (r.x0.min(r.x1).clamp(0.0, 1.0), r.x0.max(r.x1).clamp(0.0, 1.0));
        let (y0, y1) = (r.y0.min(r.y1).clamp(0.0, 1.0), r.y0.max(r.y1).clamp(0.0, 1.0));
        let cell = 1.0 / GRID as f32;
        let overlap = |lo: f32, hi: f32, i: usize| {
            ((i as f32 + 1.0) * cell).min(hi) - (i as f32 * cell).max(lo)
        };
        let (mut sum, mut weight) = (0.0f32, 0.0f32);
        for (row, cells) in self.luminance.iter().enumerate() {
            let dy = overlap(y0, y1, row);
            if dy <= 0.0 {
                continue;
            }
            for (col, lum) in cells.iter().enumerate() {
                let dx = overlap(x0, x1, col);
                if dx > 0.0 {
                    sum += lum * dx * dy;
                    weight += dx * dy;
                }
            }
        }
        if weight > 0.0 {
            return sum / weight;
        }
        // Região sem área (ponto ou linha): usa a célula sob o centro.
        let at = |c: f32| ((c * GRID as f32) as usize).min(GRID - 1);
        self.luminance[at((y0 + y1) / 2.0)][at((x0 + x1) / 2.0)]
    }

    /// Análise de fundo liso (cor sólida, sem imagem pronta).
    pub fn uniform(lum: f32, average: [f32; 3]) -> Self {
        Self { luminance: [[lum; GRID]; GRID], average }
    }
}

/// Identidade barata do que a superfície mostra; `None` enquanto o tamanho não é conhecido.
pub fn surface_tag(
    image: &widget::image::Handle,
    radius: u32,
    size: Option<(u32, u32)>,
) -> Option<Tag> {
    let monitor = size.filter(|(w, h)| *w > 0 && *h > 0)?;
    Some(Tag { image: image.id(), radius, monitor })
}

/// Desfocado só se o desfoque está ligado e o resultado pronto é do mesmo `tag`; senão o original.
pub fn pick_view_handle<'a>(
    blur: u32,
    prepared: Option<&'a Prepared>,
    tag: Tag,
    original: &'a widget::image::Handle,
) -> &'a widget::image::Handle {
    if blur == 0 {
        return original;
    }
    prepared
        .filter(|p| p.tag == tag)
        .and_then(|p| p.blurred.as_ref())
        .unwrap_or(original)
}

#[derive(Default)]
struct Slot {
    pending: Option<Tag>,
    ready: Option<Prepared>,
}

/// Resultados prontos e pedidos em andamento por superfície (`K`); descarta resultado velho.
pub struct BackdropSet<K> {
    slots: HashMap<K, Slot>,
}

impl<K> Default for BackdropSet<K> {
    fn default() -> Self {
        Self { slots: HashMap::new() }
    }
}

impl<K: Copy + Eq + Hash> BackdropSet<K> {
    /// `true` se `k` ainda não tem resultado pronto nem pedido pendente com esse `tag`.
    pub fn wants(&self, k: K, tag: Tag) -> bool {
        self.slots.get(&k).is_none_or(|slot| {
            slot.pending != Some(tag) && slot.ready.as_ref().is_none_or(|p| p.tag != tag)
        })
    }

    pub fn mark_pending(&mut self, k: K, tag: Tag) {
        self.slots.entry(k).or_default().pending = Some(tag);
    }

    /// Guarda `p` só se for o pedido pendente atual de `k`; resultado de pedido antigo é descartado.
    pub fn accept(&mut self, k: K, p: Prepared) -> bool {
        match self.slots.get_mut(&k) {
            Some(slot) if slot.pending == Some(p.tag) => {
                slot.pending = None;
                slot.ready = Some(p);
                true
            }
            _ => false,
        }
    }

    pub fn get(&self, k: K) -> Option<&Prepared> {
        self.slots.get(&k).and_then(|slot| slot.ready.as_ref())
    }

    /// Handle que a superfície `k` mostra. Sem desfoque: a original. Com o resultado do `tag` atual
    /// pronto: o desfocado dele. Com `tag` novo ainda pendente: o último desfocado pronto, para a
    /// rotação não piscar a imagem nítida. Na primeira pintura (nada pronto): a original.
    pub fn view_handle<'a>(
        &'a self,
        k: K,
        blur: u32,
        tag: Tag,
        original: &'a widget::image::Handle,
    ) -> &'a widget::image::Handle {
        if blur == 0 {
            return original;
        }
        let Some(slot) = self.slots.get(&k) else {
            return original;
        };
        slot.ready
            .as_ref()
            .filter(|p| p.tag == tag || slot.pending == Some(tag))
            .and_then(|p| p.blurred.as_ref())
            .unwrap_or(original)
    }

    /// Descarta o último resultado pronto de `k` (o pedido pendente continua marcado). Usado quando o
    /// job do `tag` novo falha: o desfocado antigo seria de outra imagem.
    pub fn drop_ready(&mut self, k: K) {
        if let Some(slot) = self.slots.get_mut(&k) {
            slot.ready = None;
        }
    }

    pub fn forget(&mut self, k: K) {
        self.slots.remove(&k);
    }
}

#[cfg(test)]
mod tests;
