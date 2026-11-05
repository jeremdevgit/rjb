// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
// Icône rJb générée procéduralement : trois disques R, G, B en synthèse additive
// sur un carré de verre. Autonome : aussi inclus par build.rs pour le .ico.

/// Rend l'icône en RGBA (taille × taille).
pub fn render_icon(size: u32) -> Vec<u8> {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    let ss = 4; // suréchantillonnage pour l'anticrénelage
    let radius = s * 0.23; // arrondi du carré
    let r_disc = s * 0.235;
    let c = s * 0.5;
    let off = s * 0.13;
    let centers = [
        (c, c - off * 1.05),                  // rouge en haut
        (c - off * 0.95, c + off * 0.62),     // vert en bas à gauche
        (c + off * 0.95, c + off * 0.62),     // bleu en bas à droite
    ];
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..ss {
                for sx in 0..ss {
                    let fx = x as f32 + (sx as f32 + 0.5) / ss as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / ss as f32;
                    let p = sample(fx, fy, s, radius, r_disc, &centers);
                    for k in 0..4 {
                        acc[k] += p[k];
                    }
                }
            }
            let n = (ss * ss) as f32;
            let i = ((y * size + x) * 4) as usize;
            let a = acc[3] / n;
            for k in 0..3 {
                // couleur non prémultipliée
                let v = if a > 0.0 { acc[k] / n / a } else { 0.0 };
                px[i + k] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
            px[i + 3] = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    px
}

/// Retourne (r, g, b, a) prémultiplié pour un point.
fn sample(x: f32, y: f32, s: f32, rad: f32, rd: f32, centers: &[(f32, f32); 3]) -> [f32; 4] {
    // carré arrondi
    let m = s * 0.04;
    let (qx, qy) = ((x - s / 2.0).abs() - (s / 2.0 - m - rad), (y - s / 2.0).abs() - (s / 2.0 - m - rad));
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
    if outside > 0.0 {
        return [0.0; 4];
    }
    let t = y / s;
    // verre sombre bleuté, plus clair en haut
    let mut col = [0.10 + 0.10 * (1.0 - t), 0.11 + 0.10 * (1.0 - t), 0.17 + 0.12 * (1.0 - t)];
    // lumière additive des trois disques, avec halo
    let tints = [[1.0, 0.16, 0.30], [0.15, 1.0, 0.42], [0.20, 0.45, 1.0]];
    for (k, &(cx, cy)) in centers.iter().enumerate() {
        let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
        let core = ((rd - d) / (s * 0.012)).clamp(0.0, 1.0);
        let glow = (-(d - rd).max(0.0) / (s * 0.06)).exp() * 0.35;
        let k_ = core + glow * (1.0 - core);
        for j in 0..3 {
            col[j] += tints[k][j] * k_;
        }
    }
    // reflet du verre en haut
    let hl = ((0.42 - t) / 0.42).clamp(0.0, 1.0);
    let band = hl * hl * 0.22;
    for v in &mut col {
        *v = (*v + band).min(1.0);
    }
    // liseré lumineux sur le bord
    let edge = (1.0 - (-outside) / (s * 0.025)).clamp(0.0, 1.0) * 0.35;
    for v in &mut col {
        *v = (*v + edge).min(1.0);
    }
    let a = 0.97;
    [col[0] * a, col[1] * a, col[2] * a, a]
}

/// Encode un .ico (images BMP 32 bits avec alpha) pour les tailles données.
#[allow(dead_code)]
pub fn ico_bytes(sizes: &[u32]) -> Vec<u8> {
    let mut images = Vec::new();
    for &sz in sizes {
        let rgba = render_icon(sz);
        let mut bmp = Vec::new();
        // BITMAPINFOHEADER (hauteur doublée : image + masque)
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(sz as i32).to_le_bytes());
        bmp.extend_from_slice(&((sz * 2) as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&[0u8; 24]);
        for y in (0..sz).rev() {
            for x in 0..sz {
                let i = ((y * sz + x) * 4) as usize;
                bmp.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        let mask_row = sz.div_ceil(32) * 4;
        bmp.extend(std::iter::repeat_n(0u8, (mask_row * sz) as usize));
        images.push((sz, bmp));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (sz, data) in &images {
        let d = if *sz >= 256 { 0 } else { *sz as u8 };
        out.extend_from_slice(&[d, d, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in images {
        out.extend_from_slice(&data);
    }
    out
}
