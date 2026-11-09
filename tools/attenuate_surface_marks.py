#!/usr/bin/env python3
"""
atenua as marcas grandes de alto contraste de uma textura de superficie, preservando o grao fino.
pensado para o snow_02 de europa, cujas manchas parecem pegadas e vento de neve terrestre.

a ideia e separar a imagem em duas partes:
    baixa = borrao gaussiano (as marcas grandes)
    alta  = imagem - baixa (o grao fino)
e atenuar so a baixa, com strength 0.0 (nao muda nada) a 1.0 (remove tudo acima da escala de
sigma). o borrao e feito por fft, que ja trata a imagem como periodica (wrap-around): a textura
repete em tile, entao a borda esquerda continua na direita e o borrao nao cria emenda.

--kind albedo: trabalha na cor, convertida de srgb para linear.
    resultado = media + (1 - strength) * (baixa - media) + alta
    a cor media por canal se mantem, porque o borrao preserva a media.

--kind normal: trabalha na altura que o normal map descreve, nao na normal. a borda de uma
pegada e um degrau afiado, que na normal aparece como alta frequencia; na altura a pegada inteira
e uma depressao de baixa frequencia.
    1. converte a normal (convencao opengl) em gradiente de altura
    2. integra o gradiente por fft (frankot-chellappa) so para achar a altura baixa
    3. gradiente_novo = gradiente_original - media - strength * gradiente(baixa)
    isso equivale a altura_nova = (1 - strength) * baixa + alta, mas sem ida e volta pela
    integracao: com strength 0.0 a normal sai igual a original (menos a inclinacao media), e a
    parte do mapa que nao e integravel (ruido do jpeg, ~3% da energia) nao se perde.
    a inclinacao media e descartada, entao NORMAL_MAP_MEAN_XY no shader cai para perto de zero.

a referencia de escala: o tile fisico do snow_02 e 2,0 m em 2048 px, cerca de 1 mm por pixel.

dependencias: numpy, pillow
uso:
    python3 tools/attenuate_surface_marks.py assets/terrain/materials/snow_02/snow_02_diff_2k.jpg
    python3 tools/attenuate_surface_marks.py assets/terrain/materials/snow_02/snow_02_nor_gl_2k.jpg --kind normal
    python3 tools/attenuate_surface_marks.py <imagem> --sigma-px 16 --strength 0.7
"""

import argparse
import re
from pathlib import Path

import numpy as np
from PIL import Image

DEFAULT_SIGMA_PX = 16.0
DEFAULT_STRENGTH = 0.7
DEFAULT_COMPARE_DIR = Path("tools/out")

# a diferenca entre original e atenuado e pequena (alguns niveis de 255); amplificada em torno do
# cinza medio para ficar visivel no painel de comparacao
DIFF_GAIN = 8.0

# cada painel da comparacao e reduzido por este fator, senao o arquivo ficaria enorme
COMPARE_DOWNSCALE = 2

# luz rasante do preview sombreado: elevacao baixa, vinda do canto superior esquerdo da imagem
PREVIEW_LIGHT_ELEVATION_DEG = 12.0
PREVIEW_LIGHT_AZIMUTH_DEG = 135.0

# protege a divisao por nz em texels quase de lado (o menor nz do snow_02 e ~0,29)
MIN_NORMAL_Z = 0.05


def srgb_to_linear(value: np.ndarray) -> np.ndarray:
    return np.where(value <= 0.04045, value / 12.92, ((value + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(value: np.ndarray) -> np.ndarray:
    value = np.clip(value, 0.0, 1.0)
    return np.where(value <= 0.0031308, value * 12.92, 1.055 * value ** (1.0 / 2.4) - 0.055)


def angular_frequencies(shape):
    """frequencias angulares (rad/px) das linhas e das colunas, na ordem da fft."""
    height, width = shape
    return 2.0 * np.pi * np.fft.fftfreq(height)[:, None], 2.0 * np.pi * np.fft.fftfreq(width)[None, :]


def gaussian_kernel(shape, sigma_px: float) -> np.ndarray:
    """transformada da gaussiana de desvio sigma (em pixels), com ganho 1 na frequencia zero."""
    omega_row, omega_col = angular_frequencies(shape)
    return np.exp(-0.5 * sigma_px**2 * (omega_col**2 + omega_row**2))


def gaussian_lowpass(channel: np.ndarray, sigma_px: float) -> np.ndarray:
    """borrao gaussiano periodico de um canal 2d, aplicado no dominio da frequencia."""
    return np.real(np.fft.ifft2(np.fft.fft2(channel) * gaussian_kernel(channel.shape, sigma_px)))


def split_bands(image: np.ndarray, sigma_px: float):
    """separa cada canal em media, parte baixa centralizada e parte alta."""
    mean = image.mean(axis=(0, 1))
    centered = image - mean
    low = np.stack([gaussian_lowpass(centered[..., c], sigma_px) for c in range(image.shape[2])], axis=-1)
    high = centered - low
    return mean, low, high


def default_output_path(input_path: Path) -> Path:
    """insere _attenuated antes do sufixo de resolucao (_2k): snow_02_nor_gl_2k -> snow_02_nor_gl_attenuated_2k."""
    stem = input_path.stem
    match = re.search(r"_(\d+k)$", stem)
    if match:
        stem = f"{stem[:match.start()]}_attenuated_{match.group(1)}"
    else:
        stem = f"{stem}_attenuated"
    return input_path.with_name(f"{stem}.png")


def to_image(values: np.ndarray) -> Image.Image:
    image = Image.fromarray(np.round(np.clip(values, 0.0, 1.0) * 255.0).astype(np.uint8))
    size = (image.width // COMPARE_DOWNSCALE, image.height // COMPARE_DOWNSCALE)
    return image.resize(size, Image.Resampling.LANCZOS)


def save_sheet(rows, path: Path):
    """monta uma grade de paineis (lista de linhas, cada linha uma lista de arrays) e salva."""
    images = [[to_image(panel) for panel in row] for row in rows]
    cell_width, cell_height = images[0][0].size
    sheet = Image.new("RGB", (cell_width * len(images[0]), cell_height * len(images)))
    for row_index, row in enumerate(images):
        for col_index, image in enumerate(row):
            sheet.paste(image, (col_index * cell_width, row_index * cell_height))
    path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(path)


def compare_path_for(compare_dir: Path, output: Path, sigma_px: float, strength: float) -> Path:
    return compare_dir / f"{output.stem}_compare_sigma{sigma_px:g}_strength{strength:g}.png"


def print_albedo_stats(label: str, image_linear: np.ndarray, sigma_px: float):
    mean, low, high = split_bands(image_linear, sigma_px)
    print(f"{label}")
    print(f"  media linear por canal (r, g, b): {np.round(mean, 4)}")
    print(f"  media srgb por canal   (r, g, b): {np.round(linear_to_srgb(mean), 4)}")
    print(f"  desvio da parte baixa  (r, g, b): {np.round(low.std(axis=(0, 1)), 4)}")
    print(f"  desvio da parte alta   (r, g, b): {np.round(high.std(axis=(0, 1)), 4)}")


def run_albedo(input_path: Path, output: Path, sigma_px: float, strength: float, compare_dir: Path):
    original_srgb = np.asarray(Image.open(input_path).convert("RGB")).astype(np.float64) / 255.0
    original_linear = srgb_to_linear(original_srgb)

    mean, low, high = split_bands(original_linear, sigma_px)
    attenuated_linear = mean + (1.0 - strength) * low + high

    clipped = np.mean((attenuated_linear < 0.0) | (attenuated_linear > 1.0))
    attenuated_srgb = np.round(linear_to_srgb(attenuated_linear) * 255.0) / 255.0

    output.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(np.round(attenuated_srgb * 255.0).astype(np.uint8)).save(output)

    compare_path = compare_path_for(compare_dir, output, sigma_px, strength)
    diff = 0.5 + DIFF_GAIN * (attenuated_srgb - original_srgb)
    save_sheet([[original_srgb, attenuated_srgb, diff]], compare_path)

    print(f"albedo, sigma = {sigma_px:g} px, strength = {strength:g}")
    print_albedo_stats("antes", original_linear, sigma_px)
    # as estatisticas de depois sao do png salvo (ja quantizado em 8 bits), nao do valor em float
    print_albedo_stats("depois", srgb_to_linear(attenuated_srgb), sigma_px)
    print(f"fracao de valores cortados em [0, 1]: {clipped:.6f}")
    return compare_path


def decode_normal_map(encoded: np.ndarray):
    """normal codificada em [0, 1] para gradiente de altura (por coluna, por linha), em altura/px.

    na convencao opengl o y positivo aponta para cima na imagem, mas a linha cresce para baixo.
    para uma superficie h, n ~ (-dh/dx, -dh/dy_cima, 1), e dh/dlinha = -dh/dy_cima, entao:
        dh/dcoluna = -nx / nz,  dh/dlinha = +ny / nz
    """
    normal = encoded * 2.0 - 1.0
    nz = np.maximum(normal[..., 2], MIN_NORMAL_Z)
    return -normal[..., 0] / nz, normal[..., 1] / nz


def encode_normal_map(gradient_col: np.ndarray, gradient_row: np.ndarray) -> np.ndarray:
    """inverso de decode_normal_map: gradiente para normal unitaria codificada em [0, 1]."""
    normal = np.stack([-gradient_col, gradient_row, np.ones_like(gradient_col)], axis=-1)
    normal /= np.linalg.norm(normal, axis=-1, keepdims=True)
    return normal * 0.5 + 0.5


def low_height_gradient(gradient_col: np.ndarray, gradient_row: np.ndarray, sigma_px: float):
    """gradiente da parte baixa da altura descrita pelo campo de gradientes.

    integra por frankot-chellappa (projecao de minimos quadrados no espaco de gradientes
    periodicos), aplica o borrao e deriva de volta, tudo no dominio da frequencia. a frequencia
    zero da altura fica indefinida e e zerada; ela nao afeta o gradiente.
    """
    omega_row, omega_col = angular_frequencies(gradient_col.shape)
    spectrum_col = np.fft.fft2(gradient_col)
    spectrum_row = np.fft.fft2(gradient_row)

    denominator = omega_col**2 + omega_row**2
    denominator[0, 0] = 1.0
    height = -1j * (omega_col * spectrum_col + omega_row * spectrum_row) / denominator
    height[0, 0] = 0.0

    low = height * gaussian_kernel(gradient_col.shape, sigma_px)
    return (
        np.real(np.fft.ifft2(1j * omega_col * low)),
        np.real(np.fft.ifft2(1j * omega_row * low)),
    )


def attenuate_normal_gradients(gradient_col, gradient_row, sigma_px: float, strength: float):
    """remove a inclinacao media e atenua a parte baixa da altura, preservando o resto do campo."""
    centered_col = gradient_col - gradient_col.mean()
    centered_row = gradient_row - gradient_row.mean()
    low_col, low_row = low_height_gradient(centered_col, centered_row, sigma_px)
    return centered_col - strength * low_col, centered_row - strength * low_row


def quantize(encoded: np.ndarray) -> np.ndarray:
    return np.round(np.clip(encoded, 0.0, 1.0) * 255.0) / 255.0


def unit_normals(encoded: np.ndarray) -> np.ndarray:
    normal = encoded * 2.0 - 1.0
    return normal / np.linalg.norm(normal, axis=-1, keepdims=True)


def angle_between_deg(encoded_a: np.ndarray, encoded_b: np.ndarray) -> np.ndarray:
    cosine = np.sum(unit_normals(encoded_a) * unit_normals(encoded_b), axis=-1)
    return np.degrees(np.arccos(np.clip(cosine, -1.0, 1.0)))


def tilt_deg(encoded: np.ndarray) -> np.ndarray:
    """inclinacao de cada texel em relacao a vertical do plano tangente."""
    return np.degrees(np.arccos(np.clip(unit_normals(encoded)[..., 2], -1.0, 1.0)))


def shaded_preview(encoded: np.ndarray) -> np.ndarray:
    """lambert com luz rasante, normalizado para que o texel plano fique cinza medio."""
    elevation = np.radians(PREVIEW_LIGHT_ELEVATION_DEG)
    azimuth = np.radians(PREVIEW_LIGHT_AZIMUTH_DEG)
    light = np.array([np.cos(elevation) * np.cos(azimuth), np.cos(elevation) * np.sin(azimuth), np.sin(elevation)])
    lambert = np.maximum(np.sum(unit_normals(encoded) * light, axis=-1), 0.0)
    gray = lambert / (2.0 * np.sin(elevation))
    return np.repeat(gray[..., None], 3, axis=-1)


def run_normal(input_path: Path, output: Path, sigma_px: float, strength: float, compare_dir: Path):
    # normal map guarda dado, nao cor: nada de conversao srgb
    original = np.asarray(Image.open(input_path).convert("RGB")).astype(np.float64) / 255.0
    gradient_col, gradient_row = decode_normal_map(original)

    # referencia para comparar: a original sem a inclinacao media, que e o que o shader ve hoje
    reference = encode_normal_map(gradient_col - gradient_col.mean(), gradient_row - gradient_row.mean())

    roundtrip = quantize(encode_normal_map(*attenuate_normal_gradients(gradient_col, gradient_row, sigma_px, 0.0)))
    roundtrip_error = angle_between_deg(roundtrip, reference)

    attenuated = quantize(encode_normal_map(*attenuate_normal_gradients(gradient_col, gradient_row, sigma_px, strength)))

    output.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(np.round(attenuated * 255.0).astype(np.uint8)).save(output)

    compare_path = compare_path_for(compare_dir, output, sigma_px, strength)
    save_sheet(
        [[original, attenuated], [shaded_preview(reference), shaded_preview(attenuated)]],
        compare_path,
    )

    def mean_xy(encoded):
        return (encoded[..., :2] * 2.0 - 1.0).mean(axis=(0, 1))

    def low_height_tilt_std(encoded):
        low_col, low_row = low_height_gradient(*decode_normal_map(encoded), sigma_px)
        return np.degrees(np.arctan(np.hypot(low_col, low_row))).std()

    after_mean = mean_xy(attenuated)
    print(f"normal map, sigma = {sigma_px:g} px, strength = {strength:g}")
    print("a) ida e volta com strength 0.0, contra a original sem a inclinacao media (8 bits):")
    print(f"   erro medio {roundtrip_error.mean():.3f} graus, p99 {np.percentile(roundtrip_error, 99):.3f}, "
          f"maximo {roundtrip_error.max():.3f}")
    print("b) media xy da normal decodificada em [-1, 1]:")
    print(f"   antes  {np.round(mean_xy(original), 4)}")
    print(f"   depois {np.round(after_mean, 4)}")
    print(f"   sugestao para o shader: NORMAL_MAP_MEAN_XY = vec2<f32>({after_mean[0]:.3f}, {after_mean[1]:.3f})")
    print("c) inclinacao por texel, em graus (antes sem a media, igual ao shader):")
    print(f"   desvio total            antes {tilt_deg(reference).std():.3f}   depois {tilt_deg(attenuated).std():.3f}")
    print(f"   desvio da parte baixa   antes {low_height_tilt_std(reference):.3f}   depois {low_height_tilt_std(attenuated):.3f}")
    return compare_path


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("input", type=Path, help="textura original (jpg ou png)")
    parser.add_argument("--kind", choices=["albedo", "normal"], default="albedo",
                        help="albedo em srgb ou normal map opengl (padrao albedo)")
    parser.add_argument("--sigma-px", type=float, default=DEFAULT_SIGMA_PX,
                        help=f"desvio do borrao, em pixels; separa marca de grao (padrao {DEFAULT_SIGMA_PX})")
    parser.add_argument("--strength", type=float, default=DEFAULT_STRENGTH,
                        help=f"quanto atenuar a parte baixa, de 0.0 a 1.0 (padrao {DEFAULT_STRENGTH})")
    parser.add_argument("--output", type=Path, default=None,
                        help="png de saida (padrao: ao lado do original, com _attenuated no nome)")
    parser.add_argument("--compare-dir", type=Path, default=DEFAULT_COMPARE_DIR,
                        help=f"pasta da imagem de comparacao (padrao {DEFAULT_COMPARE_DIR})")
    args = parser.parse_args()

    if not 0.0 <= args.strength <= 1.0:
        parser.error("--strength precisa estar entre 0.0 e 1.0")
    if args.sigma_px <= 0.0:
        parser.error("--sigma-px precisa ser positivo")

    output = args.output or default_output_path(args.input)
    if output.resolve() == args.input.resolve():
        parser.error("a saida nao pode sobrescrever o original")

    run = run_albedo if args.kind == "albedo" else run_normal
    compare_path = run(args.input, output, args.sigma_px, args.strength, args.compare_dir)

    print(f"salvo: {output} ({output.stat().st_size / 1e6:.2f} MB)")
    print(f"comparacao: {compare_path}")


if __name__ == "__main__":
    main()
