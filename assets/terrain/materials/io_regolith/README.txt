Textura de regolito de Io gerada por script (tools/generate_io_regolith.py), sem foto nem asset de
terceiros. Tile fisico de 2 m em 2048 px (cerca de 1 mm por pixel), igual ao gelo de Europa; o
shader tambem usa a mesma imagem num tile de 16 m (MACRO_SCALE_RATIO em
crates/eigc_terrain/src/surface_material.rs).

Arquivos (PNG RGB de 8 bits, 2048x2048, periodicos):
  io_regolith_diff_2k.png     albedo, em sRGB, cinza neutro (a cor vem da cor por vertice da receita)
  io_regolith_nor_gl_2k.png   normal map, convencao OpenGL
  io_regolith_arm_2k.png      AO (R), roughness (G, media ~0,86), metallic (B, sempre zero)

O relevo soma ruido fractal com o expoente de Hurst da rugosidade fina de Io (0,5, igual a
IO_DETAIL_HURST_EXPONENT em crates/eigc_terrain/src/recipe/io.rs) e clastos (domos de 1 a 10 cm,
mais escuros) espalhados. Os parametros sao constantes no topo do script.

Para regerar, a partir da raiz do repositorio (dependencias: numpy, pillow, scipy):
  python3 tools/generate_io_regolith.py

A mesma semente (--seed, padrao 501) gera os mesmos arquivos. O script tambem salva um painel de
conferencia em tools/out/ (ignorado pelo git).

A media do xy do normal map precisa ficar perto de NORMAL_MAP_MEAN_XY em
assets/shaders/terrain_detail.wgsl, que e compartilhada com Europa; o teste
surface_normal_map_mean_matches_shader (eigc_terrain) falha se nao ficar.
