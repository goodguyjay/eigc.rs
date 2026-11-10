#!/usr/bin/env python3
"""
gera a textura de gelo de europa (albedo, normal map e arm) a partir de um relevo sintetico.

o relevo e um campo de altura periodico (a textura repete em tile sem emenda), em milimetros,
com 1 pixel = 1 mm na escala curta (tile de 2 m em 2048 px). ele soma tres partes:
    1. ruido fractal com o mesmo expoente de hurst do dtm de europa (0,85): a inclinacao muda
       pouco com a escala, entao o chao parece parecido no tile de 2 m e no de 16 m (8x maior);
    2. fraturas em varias escalas, nas bordas de celulas de voronoi periodicas (pontos sorteados
       no tile inteiro, sem grade), sem direcao preferida, e so em parte das bordas (mascara de
       baixa frequencia);
    3. pequenos degraus entre as placas que as fraturas separam.

a partir da altura:
    normal map: convencao opengl (verde = +y), mesma do snow_02 e do bevy;
    albedo: gelo quase branco, um pouco mais escuro e azulado dentro das fraturas;
    arm: ao (r) pela concavidade do relevo, aspereza do gelo (g), metallic (b) sempre zero.

as tres imagens saem em png rgb de 8 bits. depois de salvar o normal map, o script relê o png e
imprime a media do xy decodificado, que e o valor de NORMAL_MAP_MEAN_XY no shader.

dependencias: numpy, pillow, scipy (cKDTree periodico para o voronoi)
uso, a partir da raiz do repositorio:
    python3 tools/generate_europa_ice.py
"""

import argparse
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.spatial import cKDTree

from attenuate_surface_marks import (
    angular_frequencies,
    encode_normal_map,
    gaussian_lowpass,
    linear_to_srgb,
    quantize,
    shaded_preview,
    srgb_to_linear,
    tilt_deg,
    unit_normals,
)

DEFAULT_SIZE_PX = 2048
DEFAULT_SEED = 204
DEFAULT_OUTPUT_DIR = Path("assets/terrain/materials/europa_ice")
DEFAULT_PREVIEW_DIR = Path("tools/out")
FILE_PREFIX = "europa_ice"

# expoente de hurst do dtm de europa (DTM_HURST_EXPONENT em recipe.rs): a amplitude de cada
# comprimento de onda cresce com ele elevado a h, e a inclinacao com h - 1, quase constante
HURST_EXPONENT = 0.85

# inclinacao rms do ruido fractal na escala de 1 px (subida/avanco)
FRACTAL_SLOPE_RMS = 0.10

# lado das celulas de voronoi de cada oitava de fraturas, em px (= mm no tile de 2 m)
CRACK_CELL_SIZES_PX = (512, 256, 128, 64)

# meia largura da fratura como fracao do lado da celula, com piso em px para nao virar serrilhado
CRACK_HALF_WIDTH_FRACTION = 0.012
CRACK_MIN_HALF_WIDTH_PX = 1.5

# profundidade da fratura como multiplo da meia largura (paredes de ~35 graus)
CRACK_DEPTH_TO_HALF_WIDTH = 0.7

# fracao das bordas de celula que viram fratura visivel, por oitava
CRACK_COVERAGE = 0.55

# desvio padrao do degrau entre placas vizinhas, como fracao do lado da celula
PLATE_STEP_FRACTION = 0.003

# albedo linear do gelo limpo (quase branco, levemente azulado) e da parte funda das fraturas
ICE_ALBEDO_LINEAR = np.array([0.80, 0.83, 0.86])
CRACK_ALBEDO_LINEAR = np.array([0.66, 0.71, 0.78])
# variacao relativa de brilho em manchas de baixa frequencia (geada mais grossa ou mais fina)
ALBEDO_MOTTLE = 0.05

# aspereza do gelo coberto de geada, variacao em manchas e quanto as fraturas (gelo exposto)
# ficam mais lisas
ROUGHNESS_BASE = 0.62
ROUGHNESS_MOTTLE = 0.08
ROUGHNESS_CRACK_DROP = 0.18

# ao pela concavidade: altura media ao redor (borrao) menos a altura local, em mm
AO_BLUR_SIGMA_PX = 6.0
AO_CAVITY_FOR_FULL_DARKNESS_MM = 4.0
AO_MIN = 0.55

# lado do recorte 1:1 do preview e lado de cada painel reduzido
PREVIEW_CROP_PX = 512
PREVIEW_PANEL_PX = 512


def fractal_noise(shape, hurst: float, rng: np.random.Generator) -> np.ndarray:
    """ruido periodico de media zero cuja amplitude por comprimento de onda segue a lei de potencia
    de hurst (espectro de amplitude ~ k^-(1 + hurst)), com fase aleatoria: sem direcao preferida."""
    omega_row, omega_col = angular_frequencies(shape)
    radius = np.sqrt(omega_row**2 + omega_col**2)
    radius[0, 0] = 1.0
    amplitude = radius ** -(1.0 + hurst)
    amplitude[0, 0] = 0.0
    phase = np.exp(2j * np.pi * rng.random(shape))
    field = np.real(np.fft.ifft2(amplitude * phase))
    return field / field.std()


def periodic_gradient(height: np.ndarray):
    """gradiente por diferenca central com volta nas bordas: (por coluna, por linha), em altura/px."""
    gradient_col = (np.roll(height, -1, axis=1) - np.roll(height, 1, axis=1)) * 0.5
    gradient_row = (np.roll(height, -1, axis=0) - np.roll(height, 1, axis=0)) * 0.5
    return gradient_col, gradient_row


def periodic_voronoi(size: int, cell_px: int, rng: np.random.Generator):
    """voronoi periodico com (size / cell_px)^2 pontos sorteados uniformemente no tile inteiro.

    devolve, por pixel, a distancia ate a borda entre as duas celulas mais proximas (pela
    mediatriz dos dois pontos), o indice da celula mais proxima e a quantidade de celulas.

    os pontos nao saem de uma grade (um ponto por quadrado): com grade, as bordas se acumulam
    perto das linhas dela e as fraturas desenham uma grade fraca alinhada aos eixos.
    """
    count = (size // cell_px) ** 2
    points = rng.random((count, 2)) * size
    rows, cols = np.mgrid[0:size, 0:size]
    pixels = np.stack([rows.ravel(), cols.ravel()], axis=-1).astype(np.float64)

    distances, indices = cKDTree(points, boxsize=size).query(pixels, k=2, workers=-1)
    separation = points[indices[:, 1]] - points[indices[:, 0]]
    separation -= size * np.round(separation / size)
    edge_distance = (distances[:, 1] ** 2 - distances[:, 0] ** 2) / (
        2.0 * np.maximum(np.linalg.norm(separation, axis=-1), 1e-6)
    )
    return edge_distance.reshape(size, size), indices[:, 0].reshape(size, size), count


def build_height(size: int, rng: np.random.Generator):
    """altura do gelo em mm e a mascara de fratura em [0, 1] (1 no fundo da fratura mais funda)."""
    shape = (size, size)
    fractal = fractal_noise(shape, HURST_EXPONENT, rng)
    gradient_col, gradient_row = periodic_gradient(fractal)
    slope_rms = np.sqrt(np.mean(gradient_col**2 + gradient_row**2))
    height = fractal * (FRACTAL_SLOPE_RMS / slope_rms)

    crack_mask = np.zeros(shape)
    for cell_px in CRACK_CELL_SIZES_PX:
        edge_distance, cell_id, cell_count = periodic_voronoi(size, cell_px, rng)
        half_width = max(cell_px * CRACK_HALF_WIDTH_FRACTION, CRACK_MIN_HALF_WIDTH_PX)

        # so parte das bordas vira fratura: mascara suave de baixa frequencia, na escala da celula
        visibility_noise = gaussian_lowpass(rng.standard_normal(shape), cell_px * 0.35)
        visibility_noise /= visibility_noise.std()
        threshold = np.quantile(visibility_noise, 1.0 - CRACK_COVERAGE)
        visibility = np.clip((visibility_noise - threshold) / 0.5 + 0.5, 0.0, 1.0)

        groove = np.exp(-((edge_distance / half_width) ** 2)) * visibility
        height -= CRACK_DEPTH_TO_HALF_WIDTH * half_width * groove
        crack_mask = np.maximum(crack_mask, groove)

        # degrau entre placas: altura constante por celula, suavizada na largura da fratura
        plate_offsets = rng.standard_normal(cell_count) * PLATE_STEP_FRACTION * cell_px
        height += gaussian_lowpass(plate_offsets[cell_id], half_width)

    return height, crack_mask


def build_albedo_linear(crack_mask: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    mottle = fractal_noise(crack_mask.shape, HURST_EXPONENT, rng)
    mottle = gaussian_lowpass(mottle, 24.0)
    mottle /= mottle.std()
    brightness = 1.0 + ALBEDO_MOTTLE * np.clip(mottle, -2.5, 2.5)
    color = ICE_ALBEDO_LINEAR + crack_mask[..., None] * (CRACK_ALBEDO_LINEAR - ICE_ALBEDO_LINEAR)
    return np.clip(color * brightness[..., None], 0.0, 1.0)


def build_arm(height: np.ndarray, crack_mask: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    cavity = np.clip((gaussian_lowpass(height, AO_BLUR_SIGMA_PX) - height) / AO_CAVITY_FOR_FULL_DARKNESS_MM, 0.0, 1.0)
    occlusion = 1.0 - (1.0 - AO_MIN) * cavity

    mottle = gaussian_lowpass(fractal_noise(height.shape, HURST_EXPONENT, rng), 16.0)
    mottle /= mottle.std()
    roughness = ROUGHNESS_BASE + ROUGHNESS_MOTTLE * np.clip(mottle, -2.0, 2.0) - ROUGHNESS_CRACK_DROP * crack_mask

    return np.stack([occlusion, np.clip(roughness, 0.0, 1.0), np.zeros_like(height)], axis=-1)


def save_rgb8(values: np.ndarray, path: Path):
    path.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(np.round(np.clip(values, 0.0, 1.0) * 255.0).astype(np.uint8), mode="RGB").save(path)


def read_rgb(path: Path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.float64) / 255.0


def saved_normal_mean_xy(path: Path):
    """media do xy decodificado para [-1, 1] no png ja salvo, igual ao teste e ao shader."""
    encoded = read_rgb(path)
    return (encoded[..., 0] * 2.0 - 1.0).mean(), (encoded[..., 1] * 2.0 - 1.0).mean()


def orientation_coherence(gradient_col: np.ndarray, gradient_row: np.ndarray) -> float:
    """coerencia do tensor de estrutura do gradiente: 0 sem direcao preferida, 1 tudo alinhado."""
    jxx, jyy, jxy = (gradient_col**2).mean(), (gradient_row**2).mean(), (gradient_col * gradient_row).mean()
    eigenvalues = np.linalg.eigvalsh([[jxx, jxy], [jxy, jyy]])
    return float((eigenvalues[1] - eigenvalues[0]) / (eigenvalues[1] + eigenvalues[0]))


def seam_percentiles(values: np.ndarray):
    """posicao (percentil) do degrau medio da borda do tile entre os degraus medios de todas as
    fronteiras internas, entre linhas e entre colunas. textura periodica: posicao qualquer; emenda
    real: sempre perto de 100. compara com cada fronteira, nao com a media, porque fraturas
    esparsas fazem o degrau de uma unica fronteira variar muito."""
    if values.ndim == 3:
        values = values.mean(axis=-1)
    result = []
    for axis in (0, 1):
        wrapped = np.concatenate([values, np.take(values, [0], axis=axis)], axis=axis)
        steps = np.abs(np.diff(wrapped, axis=axis)).mean(axis=1 - axis)
        result.append(float((steps[:-1] < steps[-1]).mean() * 100.0))
    return result


def mip_tilt_rms(encoded: np.ndarray, factor: int) -> float:
    """inclinacao rms (graus) do normal map reduzido por factor com media dos vetores, como os mips."""
    normal = unit_normals(encoded)
    size = normal.shape[0] // factor
    blocks = normal.reshape(size, factor, size, factor, 3).mean(axis=(1, 3))
    blocks /= np.linalg.norm(blocks, axis=-1, keepdims=True)
    return float(np.sqrt(np.mean(np.degrees(np.arccos(np.clip(blocks[..., 2], -1.0, 1.0))) ** 2)))


def panel(values: np.ndarray) -> Image.Image:
    image = Image.fromarray(np.round(np.clip(values, 0.0, 1.0) * 255.0).astype(np.uint8))
    if image.width != PREVIEW_PANEL_PX:
        image = image.resize((PREVIEW_PANEL_PX, PREVIEW_PANEL_PX), Image.Resampling.LANCZOS)
    return image


def save_preview(albedo_srgb: np.ndarray, normal: np.ndarray, arm: np.ndarray, path: Path):
    """linha 1: imagem inteira reduzida (albedo, normal sombreado, ao, aspereza);
    linha 2: recorte 1:1 de 512 px (~50 cm no tile de 2 m) das mesmas;
    linha 3: 2x2 tiles do normal sombreado e do albedo, para ver emenda e repeticao."""
    shaded = shaded_preview(normal)
    gray = lambda channel: np.repeat(channel[..., None], 3, axis=-1)
    crop = slice(0, PREVIEW_CROP_PX)
    rows = [
        [albedo_srgb, shaded, gray(arm[..., 0]), gray(arm[..., 1])],
        [albedo_srgb[crop, crop], shaded[crop, crop], gray(arm[crop, crop, 0]), gray(arm[crop, crop, 1])],
        [np.tile(shaded, (2, 2, 1)), np.tile(albedo_srgb, (2, 2, 1)), gray(np.tile(arm[..., 0], (2, 2))), shaded * albedo_srgb],
    ]
    panels = [[panel(values) for values in row] for row in rows]
    sheet = Image.new("RGB", (PREVIEW_PANEL_PX * 4, PREVIEW_PANEL_PX * len(panels)))
    for row_index, row in enumerate(panels):
        for col_index, image in enumerate(row):
            sheet.paste(image, (col_index * PREVIEW_PANEL_PX, row_index * PREVIEW_PANEL_PX))
    path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED, help="semente do gerador (mesma semente, mesmas imagens)")
    parser.add_argument("--size", type=int, default=DEFAULT_SIZE_PX, help="lado da textura em px (multiplo de 512)")
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR, help="pasta das tres imagens")
    parser.add_argument("--preview-dir", type=Path, default=DEFAULT_PREVIEW_DIR, help="pasta do painel de conferencia")
    args = parser.parse_args()

    rng = np.random.default_rng(args.seed)
    height, crack_mask = build_height(args.size, rng)
    gradient_col, gradient_row = periodic_gradient(height)
    normal = quantize(encode_normal_map(gradient_col, gradient_row))
    albedo_srgb = quantize(linear_to_srgb(build_albedo_linear(crack_mask, rng)))
    arm = quantize(build_arm(height, crack_mask, rng))

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
    print(f"fracao da area com fratura (mascara > 0,5): {(crack_mask > 0.5).mean():.3f}")

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
    print(f"NORMAL_MAP_MEAN_XY: vec2<f32>({mean_x:.3f}, {mean_y:.3f})")


if __name__ == "__main__":
    main()
