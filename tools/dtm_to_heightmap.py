#!/usr/bin/env python3
"""
converte um DTM real da usgs (galileo ssi stereophotogrammetry) num heightmap
16-bit pronto pra consumo em eigc_terrain.

fonte dos dados: bucket publico s3 "astrogeo-ard" (usgs astrogeology / nasa
open data).

dependencias: numpy, pillow, tifffile, imagecodecs
    pip install numpy pillow tifffile imagecodecs

uso:
    python3 dtm_to_heightmap.py Rhadamanthys --crop 60 230 210 380

o --crop (y0 x0 y1 x1) deve cair inteiramente dentro da regiao valida do dtm
(o dtm e entregue como um paralelogramo rotacionado dentro de uma grade
retangular maior, a area fora do paralelogramo e nodata). rode primeiro sem
--crop pra inspecionar a mascara de validade antes de escolher a janela.
"""

import argparse
import json
import urllib.request
from pathlib import Path

import numpy as np
import tifffile
from PIL import Image

BASE_URL = "https://astrogeo-ard.s3.amazonaws.com/jupiter/europa/galileo_voyager/usgs_controlled_dtms"
NODATA_THRESHOLD = -1e30  # sentinela gdal e ~-3.4e38, qualquer coisa abaixo disso e nodata


def download(site: str, out_dir: Path) -> Path:
    """baixa o .tif do site indicado (ex: 'Rhadamanthys', 'Yelland_Ridges') se ainda nao existir."""
    out_dir.mkdir(parents=True, exist_ok=True)
    dest = out_dir / f"{site}.tif"
    if not dest.exists():
        url = f"{BASE_URL}/{site}/{site}.tif"
        urllib.request.urlretrieve(url, dest)
    return dest


def read_elevation(tif_path: Path) -> tuple[np.ndarray, float]:
    """le o raster de elevacao (metros) e retorna (array, metros_por_pixel)."""
    arr = tifffile.imread(tif_path).astype(np.float64)
    tags = {t.name: t.value for t in tifffile.TiffFile(tif_path).pages[0].tags}
    px_scale_m = float(tags["ModelPixelScaleTag"][0])
    return arr, px_scale_m


def find_valid_square(arr: np.ndarray, max_size: int = 250, step: int = 10) -> tuple[int, int, int, int]:
    """varredura simples pra achar um recorte quadrado totalmente livre de nodata.
    nao e otimo, e so um ponto de partida: inspecione visualmente antes de aceitar."""
    mask = arr > NODATA_THRESHOLD
    h, w = arr.shape
    best = None
    for size in range(max_size, 50, -step):
        for y0 in range(0, h - size, step):
            for x0 in range(0, w - size, step):
                if mask[y0 : y0 + size, x0 : x0 + size].all():
                    best = (y0, x0, y0 + size, x0 + size)
                    return best
    if best is None:
        raise RuntimeError("nenhum recorte totalmente valido encontrado nesse tamanho minimo")
    return best


def write_ron_metadata(path: Path, meta: dict) -> None:
    """escreve o sidecar no mesmo formato ron usado pelos moon profiles do projeto."""
    lines = ["("]
    for k, v in meta.items():
        if isinstance(v, str):
            lines.append(f'    {k}: "{v}",')
        else:
            lines.append(f"    {k}: {v},")
    lines.append(")")
    path.write_text("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("site", help="nome do site no bucket usgs (ex: Rhadamanthys, Yelland_Ridges, Agenor, Cilix_Crater, Pwyll_Crater)")
    parser.add_argument("--crop", nargs=4, type=int, metavar=("Y0", "X0", "Y1", "X1"), help="janela de recorte em pixels da fonte; se omitido, procura automaticamente a maior janela 100% valida")
    parser.add_argument("--out-dir", default=".", help="diretorio de saida")
    parser.add_argument("--raw-dir", default="./raw_dtms", help="diretorio de cache dos .tif baixados")
    args = parser.parse_args()

    raw_dir = Path(args.raw_dir)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    tif_path = download(args.site, raw_dir)
    arr, px_scale_m = read_elevation(tif_path)
    mask = arr > NODATA_THRESHOLD

    if args.crop:
        y0, x0, y1, x1 = args.crop
        if not mask[y0:y1, x0:x1].all():
            raise SystemExit("o recorte pedido contem nodata. rode sem --crop pra ver uma janela valida sugerida, ou ajuste manualmente.")
    else:
        y0, x0, y1, x1 = find_valid_square(arr)
        print(f"nenhum --crop informado. sugestao automatica (inspecione antes de usar): {y0} {x0} {y1} {x1}")

    patch = arr[y0:y1, x0:x1]
    elev_min, elev_max = float(patch.min()), float(patch.max())
    norm = (patch - elev_min) / (elev_max - elev_min)
    png16 = (norm * 65535).astype(np.uint16)

    stem = f"europa_{args.site.lower()}_patch"
    png_path = out_dir / f"{stem}_heightmap.png"
    ron_path = out_dir / f"{stem}_heightmap.ron"

    Image.fromarray(png16).save(png_path)  # pillow infere I;16 a partir do dtype uint16
    write_ron_metadata(
        ron_path,
        {
            "source": f"NASA/USGS Controlled Europa DTMs (Galileo SSI stereophotogrammetry, SOCET SET), site: {args.site}",
            "crs": "IAU_2015:50215",
            "pixel_scale_m": px_scale_m,
            "patch_width_px": x1 - x0,
            "patch_height_px": y1 - y0,
            "patch_width_m": (x1 - x0) * px_scale_m,
            "patch_height_m": (y1 - y0) * px_scale_m,
            "elevation_min_m": elev_min,
            "elevation_max_m": elev_max,
            "crop_in_source_px": f"({y0}, {x0}, {y1}, {x1})",
        },
    )

    print(f"heightmap: {png_path}")
    print(f"metadata:  {ron_path}")
    print(f"patch real: {(x1-x0)*px_scale_m/1000:.2f} km x {(y1-y0)*px_scale_m/1000:.2f} km, "
          f"elevacao {elev_min:.1f} a {elev_max:.1f} m (range {elev_max-elev_min:.1f} m)")


if __name__ == "__main__":
    main()
