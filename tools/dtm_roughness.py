#!/usr/bin/env python3
"""
mede a rugosidade de um heightmap gerado por dtm_to_heightmap.py: a diferenca rms de altura
entre pontos separados por uma distancia L, e o expoente de hurst H (rms ~ L^H).

os valores de europa_recipe (DTM_HURST_EXPONENT, DTM_ROUGHNESS_AT_PIXEL_M em recipe/europa.rs) vem
daqui. o ajuste usa distancias curtas (1 a 12 pixels) porque acima disso a curva satura.

dependencias: numpy, pillow
uso:
    python3 tools/dtm_roughness.py assets/terrain/europa/europa_rhadamanthys_patch_heightmap
"""

import argparse
import re
from pathlib import Path

import numpy as np
from PIL import Image

LAGS_PX = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32]
FIT_MAX_INDEX = 7  # ajusta com lags de 1 a 12 pixels


def read_ron_number(text: str, key: str) -> float:
    return float(re.search(rf"{key}:\s*(-?[0-9.eE+-]+)", text).group(1))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("stem", help="caminho sem extensao (le <stem>.png e <stem>.ron)")
    args = parser.parse_args()

    ron = Path(f"{args.stem}.ron").read_text()
    elev_min = read_ron_number(ron, "elevation_min_m")
    elev_max = read_ron_number(ron, "elevation_max_m")
    pixel_m = read_ron_number(ron, "pixel_scale_m")

    raw = np.array(Image.open(f"{args.stem}.png")).astype(np.float64)
    h = elev_min + raw / 65535.0 * (elev_max - elev_min)

    rms = []
    for lag in LAGS_PX:
        dx = h[:, lag:] - h[:, :-lag]
        dz = h[lag:, :] - h[:-lag, :]
        rms.append(np.sqrt(0.5 * (np.mean(dx**2) + np.mean(dz**2))))
        print(f"lag {lag:3d} px ({lag * pixel_m:8.1f} m)  rms dh = {rms[-1]:7.2f} m")

    log_lag = np.log(np.array(LAGS_PX[:FIT_MAX_INDEX]) * pixel_m)
    slope, intercept = np.polyfit(log_lag, np.log(rms[:FIT_MAX_INDEX]), 1)
    print(f"H = {slope:.3f}, rms a 1 pixel = {np.exp(slope * np.log(pixel_m) + intercept):.2f} m")


if __name__ == "__main__":
    main()
