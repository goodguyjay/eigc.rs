#!/usr/bin/env python3
"""
gera a textura de regolito de ganimedes (albedo, normal map e arm) a partir de um relevo sintetico.

mesmo formato do gelo de europa e do regolito de io (tools/generate_europa_ice.py,
tools/generate_io_regolith.py): campo de altura periodico em milimetros, 1 pixel = 1 mm na escala
curta (tile de 2 m em 2048 px). o relevo soma duas partes:
    1. ruido fractal com o mesmo expoente de hurst da rugosidade fina de ganimedes
       (GANYMEDE_DETAIL_HURST_EXPONENT em recipe/ganymede.rs, 0,5): po fino sobre gelo;
    2. blocos de gelo espalhados em quatro tamanhos, de ~1 a ~8 cm, mais numerosos quanto
       menores: no terreno escuro, uma camada fina de material escuro (poeira, silicatos) cobre o
       gelo, e o que aflora sao blocos de gelo mais limpo, mais claros que o po. e o inverso de io,
       onde os clastos de silicato sao os escuros.

a partir da altura:
    normal map: convencao opengl (verde = +y), mesma do bevy;
    albedo: cinza neutro, porque o shader multiplica pela cor da receita (terreno escuro, faixa
        clara, crateras recentes, geada); os blocos de gelo sao mais claros;
    arm: ao (r) pela concavidade do relevo, aspereza (g) em torno do perceptual_roughness do
        ganymede.ron (0,8), mais lisa nos blocos de gelo, metallic (b) sempre zero.

o xy medio do normal map precisa ficar perto de zero: NORMAL_MAP_MEAN_XY no shader e uma
constante so, compartilhada com europa e io. o relevo e periodico e sem inclinacao media, entao
isso vale por construcao; o script imprime a media medida no png salvo, e o teste
surface_normal_map_mean_matches_shader confere.

dependencias: numpy, pillow, scipy
uso, a partir da raiz do repositorio:
    python3 tools/generate_ganymede_regolith.py
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
# escolhida entre 307 e 312 pela emenda do tile mais equilibrada nos tres mapas (normal 36/40,
# perto da de io); a 307 dava 97/85 no normal map
DEFAULT_SEED = 311
DEFAULT_OUTPUT_DIR = Path("assets/terrain/materials/ganymede_regolith")
DEFAULT_PREVIEW_DIR = Path("tools/out")
FILE_PREFIX = "ganymede_regolith"

# expoente de hurst da rugosidade fina de ganimedes (GANYMEDE_DETAIL_HURST_EXPONENT em
# recipe/ganymede.rs)
HURST_EXPONENT = 0.5

# inclinacao rms do ruido fractal na escala de 1 px (subida/avanco). um pouco menor que a de io
# (0,18): po fino assentado sobre gelo, menos aspero que derrame de lava
FRACTAL_SLOPE_RMS = 0.15

# blocos de gelo: (raio em px = mm, quantidade no tile de 2 m)
BLOCK_SIZES = ((40, 10), (20, 40), (10, 160), (5, 640))

# altura do bloco como fracao do raio (blocos meio enterrados no po)
BLOCK_HEIGHT_TO_RADIUS = 0.5

# variacao relativa da altura de cada bloco
BLOCK_HEIGHT_SPREAD = 0.5

# altura relativa ao pico nominal onde o bloco comeca a clarear o albedo e onde clareia todo
BLOCK_MASK_START = 0.35
BLOCK_MASK_END = 0.85

# albedo linear do po (cinza neutro, com media perto da de io, ~0,88, para escurecer a cor da
# receita na mesma medida) e dos blocos de gelo (mais claros)
DUST_ALBEDO_LINEAR = np.array([0.87, 0.87, 0.87])
BLOCK_ALBEDO_LINEAR = np.array([1.0, 1.0, 1.0])
# variacao relativa de brilho em manchas de baixa frequencia (camada escura mais grossa ou fina)
ALBEDO_MOTTLE = 0.06

# aspereza do po, variacao em manchas e quanto o gelo dos blocos e mais liso
ROUGHNESS_BASE = 0.82
ROUGHNESS_MOTTLE = 0.05
ROUGHNESS_BLOCK_DROP = 0.15

# ao pela concavidade: altura media ao redor (borrao) menos a altura local, em mm
AO_BLUR_SIGMA_PX = 8.0
AO_CAVITY_FOR_FULL_DARKNESS_MM = 6.0
AO_MIN = 0.55


def block_field(size: int, radius_px: int, count: int, rng: np.random.Generator):
    """domos gaussianos periodicos em posicoes sorteadas: altura em mm e mascara em [0, 1]."""
    impulses = np.zeros((size, size))
    rows = rng.integers(0, size, count)
    cols = rng.integers(0, size, count)
    heights = radius_px * BLOCK_HEIGHT_TO_RADIUS * (1.0 + BLOCK_HEIGHT_SPREAD * (rng.random(count) * 2.0 - 1.0))
    np.add.at(impulses, (rows, cols), heights)

    # o borrao gaussiano de um impulso unitario tem pico 1 / (2 pi sigma^2)
    sigma = radius_px / 1.5
    height = gaussian_lowpass(impulses, sigma) * (2.0 * np.pi * sigma**2)
    # so o topo do bloco aflora limpo (o resto fica coberto de po), com borda suave
    relative = height / (radius_px * BLOCK_HEIGHT_TO_RADIUS)
    mask = np.clip((relative - BLOCK_MASK_START) / (BLOCK_MASK_END - BLOCK_MASK_START), 0.0, 1.0)
    return height, mask


def build_height(size: int, rng: np.random.Generator):
    """altura do regolito em mm e a mascara dos blocos de gelo em [0, 1]."""
    fractal = fractal_noise((size, size), HURST_EXPONENT, rng)
    gradient_col, gradient_row = periodic_gradient(fractal)
    slope_rms = np.sqrt(np.mean(gradient_col**2 + gradient_row**2))
    height = fractal * (FRACTAL_SLOPE_RMS / slope_rms)

    block_mask = np.zeros((size, size))
    for radius_px, count in BLOCK_SIZES:
        scaled_count = int(round(count * (size / DEFAULT_SIZE_PX) ** 2))
        block_height, mask = block_field(size, radius_px, scaled_count, rng)
        height += block_height
        block_mask = np.maximum(block_mask, mask)

    return height, block_mask


def build_albedo_linear(block_mask: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    mottle = gaussian_lowpass(fractal_noise(block_mask.shape, HURST_EXPONENT, rng), 24.0)
    mottle /= mottle.std()
    brightness = 1.0 + ALBEDO_MOTTLE * np.clip(mottle, -2.5, 2.5)
    color = DUST_ALBEDO_LINEAR + block_mask[..., None] * (BLOCK_ALBEDO_LINEAR - DUST_ALBEDO_LINEAR)
    return np.clip(color * brightness[..., None], 0.0, 1.0)


def build_arm(height: np.ndarray, block_mask: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    cavity = np.clip((gaussian_lowpass(height, AO_BLUR_SIGMA_PX) - height) / AO_CAVITY_FOR_FULL_DARKNESS_MM, 0.0, 1.0)
    occlusion = 1.0 - (1.0 - AO_MIN) * cavity

    mottle = gaussian_lowpass(fractal_noise(height.shape, HURST_EXPONENT, rng), 16.0)
    mottle /= mottle.std()
    roughness = ROUGHNESS_BASE + ROUGHNESS_MOTTLE * np.clip(mottle, -2.0, 2.0) - ROUGHNESS_BLOCK_DROP * block_mask

    return np.stack([occlusion, np.clip(roughness, 0.0, 1.0), np.zeros_like(height)], axis=-1)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED, help="semente do gerador (mesma semente, mesmas imagens)")
    parser.add_argument("--size", type=int, default=DEFAULT_SIZE_PX, help="lado da textura em px (multiplo de 512)")
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR, help="pasta das tres imagens")
    parser.add_argument("--preview-dir", type=Path, default=DEFAULT_PREVIEW_DIR, help="pasta do painel de conferencia")
    args = parser.parse_args()

    rng = np.random.default_rng(args.seed)
    height, block_mask = build_height(args.size, rng)
    gradient_col, gradient_row = periodic_gradient(height)
    normal = quantize(encode_normal_map(gradient_col, gradient_row))
    albedo_srgb = quantize(linear_to_srgb(build_albedo_linear(block_mask, rng)))
    arm = quantize(build_arm(height, block_mask, rng))

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
    print(f"fracao da area com bloco de gelo (mascara > 0,5): {(block_mask > 0.5).mean():.3f}")

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
    print("precisa ficar a ate 0,003 de NORMAL_MAP_MEAN_XY no shader, compartilhado com europa e io")


if __name__ == "__main__":
    main()
