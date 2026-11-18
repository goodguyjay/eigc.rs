Textura de regolito de Ganimedes gerada por script (tools/generate_ganymede_regolith.py), sem foto
nem asset de terceiros. Tile fisico de 2 m em 2048 px (cerca de 1 mm por pixel), igual ao gelo de
Europa e ao regolito de Io; o shader tambem usa a mesma imagem num tile de 16 m (MACRO_SCALE_RATIO
em crates/eigc_terrain/src/surface_material.rs).

Arquivos (PNG RGB de 8 bits, 2048x2048, periodicos):
  ganymede_regolith_diff_2k.png     albedo, em sRGB, cinza neutro (a cor vem do mapa de cor da receita)
  ganymede_regolith_nor_gl_2k.png   normal map, convencao OpenGL
  ganymede_regolith_arm_2k.png      AO (R), roughness (G, media ~0,82), metallic (B, sempre zero)

O relevo soma ruido fractal com o expoente de Hurst da rugosidade fina de Ganimedes (0,5, igual a
GANYMEDE_DETAIL_HURST_EXPONENT em crates/eigc_terrain/src/recipe/ganymede.rs) e blocos de gelo
(domos de 1 a 8 cm, mais claros e mais lisos) espalhados: no terreno escuro, uma camada fina de
material escuro cobre o gelo, e o que aflora e gelo mais limpo. Os parametros sao constantes no
topo do script.

Para regerar, a partir da raiz do repositorio (dependencias: numpy, pillow, scipy):
  python3 tools/generate_ganymede_regolith.py

A mesma semente (--seed, padrao 311) gera os mesmos arquivos. O script tambem salva um painel de
conferencia em tools/out/ (ignorado pelo git).

A media do xy do normal map precisa ficar perto de NORMAL_MAP_MEAN_XY em
assets/shaders/terrain_detail.wgsl, que e compartilhada com Europa e Io; o teste
surface_normal_map_mean_matches_shader (eigc_terrain) falha se nao ficar.
