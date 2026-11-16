#!/usr/bin/env python3
"""
gera a textura de regolito de io (albedo, normal map e arm) a partir de um relevo sintetico.

mesmo formato do gelo de europa (tools/generate_europa_ice.py): campo de altura periodico em
milimetros, 1 pixel = 1 mm na escala curta (tile de 2 m em 2048 px). o relevo soma duas partes:
    1. ruido fractal com o mesmo expoente de hurst da rugosidade fina de io
       (IO_DETAIL_HURST_EXPONENT em recipe/io.rs, 0,5): chao aspero, de derrame de lava coberto de
       po de enxofre;
    2. fragmentos (clastos) espalhados em quatro tamanhos, de ~1 a ~10 cm, mais numerosos quanto
       menores: piroclastos e pedacos de lava solidificada.

a partir da altura:
    normal map: convencao opengl (verde = +y), mesma do bevy;
    albedo: cinza claro neutro, porque o shader multiplica pela cor por vertice da receita (que da
        o amarelo do enxofre, a geada e o halo); os clastos de silicato sao mais escuros;
    arm: ao (r) pela concavidade do relevo, aspereza (g) em torno do perceptual_roughness do
        io.ron (0,85), metallic (b) sempre zero.

o xy medio do normal map precisa ficar perto de zero: NORMAL_MAP_MEAN_XY no shader e uma
constante so, compartilhada com europa. o relevo e periodico e sem inclinacao media, entao isso
vale por construcao; o script imprime a media medida no png salvo, e o teste
surface_normal_map_mean_matches_shader confere.

dependencias: numpy, pillow, scipy
uso, a partir da raiz do repositorio:
    python3 tools/generate_io_regolith.py
"""

import argparse
from pathlib import Path

import numpy as np

from attenuate_surface_marks import (
    encode_normal_map,
    gaussian_lowpass,
    linear_to_srgb,
    quantize,
    srgb_to_linear,
    tilt_deg,
)
from generate_europa_ice import (
    fractal_noise,
    mip_tilt_rms,
    orientation_coherence,
    periodic_gradient,
    save_preview,
    save_rgb8,
    saved_normal_mean_xy,
    seam_percentiles,
)
from PIL import Image

DEFAULT_SIZE_PX = 2048
DEFAULT_SEED = 501
DEFAULT_OUTPUT_DIR = Path("assets/terrain/materials/io_regolith")
DEFAULT_PREVIEW_DIR = Path("tools/out")
FILE_PREFIX = "io_regolith"

# expoente de hurst da rugosidade fina de io (IO_DETAIL_HURST_EXPONENT em recipe/io.rs)
HURST_EXPONENT = 0.5

# inclinacao rms do ruido fractal na escala de 1 px (subida/avanco). maior que a de europa (0,10)
# porque com hurst 0,5 a inclinacao cai mais rapido com a escala, e o relevo ainda precisa
# aparecer no tile de 16 m
FRACTAL_SLOPE_RMS = 0.18

# clastos: (raio em px = mm, quantidade no tile de 2 m)
CLAST_SIZES = ((50, 12), (25, 50), (12, 200), (6, 800))

# altura do clasto como fracao do raio (domos baixos, meio enterrados no po)
CLAST_HEIGHT_TO_RADIUS = 0.45

# variacao relativa da altura de cada clasto
CLAST_HEIGHT_SPREAD = 0.5

# altura relativa ao pico nominal onde o clasto comeca a escurecer o albedo e onde escurece todo
CLAST_MASK_START = 0.35
CLAST_MASK_END = 0.85

# albedo linear do po (cinza neutro claro) e dos clastos de silicato
DUST_ALBEDO_LINEAR = np.array([0.90, 0.90, 0.90])
CLAST_ALBEDO_LINEAR = np.array([0.50, 0.50, 0.50])
# variacao relativa de brilho em manchas de baixa frequencia (po mais grosso ou mais fino)
ALBEDO_MOTTLE = 0.06

# aspereza do po, variacao em manchas e quanto os clastos sao mais lisos
ROUGHNESS_BASE = 0.86
ROUGHNESS_MOTTLE = 0.05
ROUGHNESS_CLAST_DROP = 0.12

# ao pela concavidade: altura media ao redor (borrao) menos a altura local, em mm
AO_BLUR_SIGMA_PX = 8.0
AO_CAVITY_FOR_FULL_DARKNESS_MM = 6.0
AO_MIN = 0.55


def clast_field(size: int, radius_px: int, count: int, rng: np.random.Generator):
    """domos gaussianos periodicos em posicoes sorteadas: altura em mm e mascara em [0, 1]."""
    impulses = np.zeros((size, size))
    rows = rng.integers(0, size, count)
    cols = rng.integers(0, size, count)
    heights = radius_px * CLAST_HEIGHT_TO_RADIUS * (1.0 + CLAST_HEIGHT_SPREAD * (rng.random(count) * 2.0 - 1.0))
    np.add.at(impulses, (rows, cols), heights)

    # o borrao gaussiano de um impulso unitario tem pico 1 / (2 pi sigma^2)
    sigma = radius_px / 1.5
    height = gaussian_lowpass(impulses, sigma) * (2.0 * np.pi * sigma**2)
    # so o topo do domo escurece (o resto fica coberto de po), com borda suave
    relative = height / (radius_px * CLAST_HEIGHT_TO_RADIUS)
    mask = np.clip((relative - CLAST_MASK_START) / (CLAST_MASK_END - CLAST_MASK_START), 0.0, 1.0)
    return height, mask


def build_height(size: int, rng: np.random.Generator):
    """altura do regolito em mm e a mascara dos clastos em [0, 1]."""
    fractal = fractal_noise((size, size), HURST_EXPONENT, rng)
    gradient_col, gradient_row = periodic_gradient(fractal)
    slope_rms = np.sqrt(np.mean(gradient_col**2 + gradient_row**2))
    height = fractal * (FRACTAL_SLOPE_RMS / slope_rms)

    clast_mask = np.zeros((size, size))
    for radius_px, count in CLAST_SIZES:
        scaled_count = int(round(count * (size / DEFAULT_SIZE_PX) ** 2))
        clast_height, mask = clast_field(size, radius_px, scaled_count, rng)
        height += clast_height
        clast_mask = np.maximum(clast_mask, mask)

    return height, clast_mask


def build_albedo_linear(clast_mask: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    mottle = gaussian_lowpass(fractal_noise(clast_mask.shape, HURST_EXPONENT, rng), 24.0)
    mottle /= mottle.std()
    brightness = 1.0 + ALBEDO_MOTTLE * np.clip(mottle, -2.5, 2.5)
    color = DUST_ALBEDO_LINEAR + clast_mask[..., None] * (CLAST_ALBEDO_LINEAR - DUST_ALBEDO_LINEAR)
    return np.clip(color * brightness[..., None], 0.0, 1.0)


def build_arm(height: np.ndarray, clast_mask: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    cavity = np.clip((gaussian_lowpass(height, AO_BLUR_SIGMA_PX) - height) / AO_CAVITY_FOR_FULL_DARKNESS_MM, 0.0, 1.0)
    occlusion = 1.0 - (1.0 - AO_MIN) * cavity

    mottle = gaussian_lowpass(fractal_noise(height.shape, HURST_EXPONENT, rng), 16.0)
    mottle /= mottle.std()
    roughness = ROUGHNESS_BASE + ROUGHNESS_MOTTLE * np.clip(mottle, -2.0, 2.0) - ROUGHNESS_CLAST_DROP * clast_mask

    return np.stack([occlusion, np.clip(roughness, 0.0, 1.0), np.zeros_like(height)], axis=-1)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED, help="semente do gerador (mesma semente, mesmas imagens)")
    parser.add_argument("--size", type=int, default=DEFAULT_SIZE_PX, help="lado da textura em px (multiplo de 512)")
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR, help="pasta das tres imagens")
    parser.add_argument("--preview-dir", type=Path, default=DEFAULT_PREVIEW_DIR, help="pasta do painel de conferencia")
    args = parser.parse_args()

    rng = np.random.default_rng(args.seed)
    height, clast_mask = build_height(args.size, rng)
    gradient_col, gradient_row = periodic_gradient(height)
    normal = quantize(encode_normal_map(gradient_col, gradient_row))
    albedo_srgb = quantize(linear_to_srgb(build_albedo_linear(clast_mask, rng)))
    arm = quantize(build_arm(height, clast_mask, rng))

    suffix = f"{args.size // 1024}k"
    paths = {
        "albedo": args.output_dir / f"{FILE_PREFIX}_diff_{suffix}.png",
        "normal": args.output_dir / f"{FILE_PREFIX}_nor_gl_{suffix}.png",
        "arm": args.output_dir / f"{FILE_PREFIX}_arm_{suffix}.png",
    }
    save_rgb8(albedo_srgb, paths["albedo"])
    save_rgb8(normal, paths["normal"])
    save_rgb8(arm, paths["arm"])
    preview_path = args.preview_dir / f"{FILE_PREFIX}_preview_seed{args.seed}.png"
    save_preview(albedo_srgb, normal, arm, preview_path)

    for name, path in paths.items():
        with Image.open(path) as image:
            print(f"{name:7s} {path}  {image.size[0]}x{image.size[1]} {image.mode}  {path.stat().st_size / 1e6:.1f} MB")
    print(f"preview {preview_path}")

    albedo_linear = srgb_to_linear(albedo_srgb)
    print("\nalbedo (linear): media", np.round(albedo_linear.mean(axis=(0, 1)), 3),
          "min", np.round(albedo_linear.min(axis=(0, 1)), 3), "max", np.round(albedo_linear.max(axis=(0, 1)), 3))
    print(f"arm: ao {arm[..., 0].min():.2f} a {arm[..., 0].max():.2f} (media {arm[..., 0].mean():.2f}), "
          f"aspereza {arm[..., 1].min():.2f} a {arm[..., 1].max():.2f} (media {arm[..., 1].mean():.2f}), "
          f"metallic max {arm[..., 2].max():.3f}")
    print(f"fracao da area com clasto (mascara > 0,5): {(clast_mask > 0.5).mean():.3f}")

    tilt = tilt_deg(normal)
    print(f"\ninclinacao do normal: media {tilt.mean():.1f} graus, p95 {np.percentile(tilt, 95):.1f}, max {tilt.max():.1f}")
    print(f"inclinacao rms por nivel de mip: nivel 0 {mip_tilt_rms(normal, 1):.1f} graus, "
          f"nivel 3 (8x, tile de 16 m) {mip_tilt_rms(normal, 8):.1f} graus")

    luminance_col, luminance_row = periodic_gradient(albedo_linear.mean(axis=-1))
    print(f"\ncoerencia de direcao (0 = sem direcao): relevo {orientation_coherence(gradient_col, gradient_row):.3f}, "
          f"albedo {orientation_coherence(luminance_col, luminance_row):.3f}")
    print("emenda do tile, percentil da borda entre as fronteiras internas (linhas, colunas):")
    for name, values in (("albedo", albedo_srgb), ("normal", normal), ("arm", arm)):
        row_percentile, col_percentile = seam_percentiles(values)
        print(f"   {name:6s} {row_percentile:.0f}, {col_percentile:.0f}")

    mean_x, mean_y = saved_normal_mean_xy(paths["normal"])
    print(f"\nmedia do xy do normal map salvo: ({mean_x:.4f}, {mean_y:.4f})")
    print("precisa ficar a ate 0,003 de NORMAL_MAP_MEAN_XY no shader, compartilhado com europa")


if __name__ == "__main__":
    main()
