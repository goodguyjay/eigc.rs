Textura de gelo de Europa gerada por script (tools/generate_europa_ice.py), sem foto nem asset de
terceiros. Tile fisico de 2 m em 2048 px (cerca de 1 mm por pixel); o shader tambem usa a mesma
imagem num tile de 16 m (MACRO_SCALE_RATIO em crates/eigc_terrain/src/surface_material.rs).

Arquivos (PNG RGB de 8 bits, 2048x2048, periodicos):
  europa_ice_diff_2k.png     albedo, em sRGB
  europa_ice_nor_gl_2k.png   normal map, convencao OpenGL
  europa_ice_arm_2k.png      AO (R), roughness (G), metallic (B, sempre zero)

O relevo soma ruido fractal com o expoente de Hurst do DTM de Europa (0,85), fraturas de Voronoi
periodico em quatro escalas (celulas de 51, 26, 13 e 6 cm) e pequenos degraus entre as placas.
Os parametros sao constantes no topo do script.

Para regerar, a partir da raiz do repositorio (dependencias: numpy, pillow, scipy):
  python3 tools/generate_europa_ice.py

A mesma semente (--seed, padrao 204) gera os mesmos arquivos. O script tambem salva um painel de
conferencia em tools/out/ (ignorado pelo git).

Depois de regerar, atualizar NORMAL_MAP_MEAN_XY em assets/shaders/terrain_detail.wgsl com o valor
que o script imprime; o teste surface_normal_map_mean_matches_shader (eigc_terrain) falha se a
constante ficar desatualizada.
