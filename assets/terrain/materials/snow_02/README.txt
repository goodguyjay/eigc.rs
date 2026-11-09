Texturas snow_02 baixadas do Poly Haven (https://polyhaven.com/a/snow_02), licenca CC0.

Originais (fonte, nao editar):
  snow_02_diff_2k.jpg     albedo
  snow_02_nor_gl_2k.jpg   normal map, convencao OpenGL
  snow_02_arm_2k.jpg      AO (R), roughness (G), metallic (B); usado pelo jogo sem alteracao

Derivados (usados pelo jogo), com as marcas grandes de alto contraste atenuadas por
tools/attenuate_surface_marks.py com --sigma-px 16 --strength 0.7:
  snow_02_diff_attenuated_2k.png
  snow_02_nor_gl_attenuated_2k.png

Para regerar, a partir da raiz do repositorio (dependencias: numpy, pillow):
  python3 tools/attenuate_surface_marks.py assets/terrain/materials/snow_02/snow_02_diff_2k.jpg --sigma-px 16 --strength 0.7
  python3 tools/attenuate_surface_marks.py assets/terrain/materials/snow_02/snow_02_nor_gl_2k.jpg --kind normal --sigma-px 16 --strength 0.7

Depois de regerar o normal map, atualizar NORMAL_MAP_MEAN_XY em assets/shaders/terrain_detail.wgsl
com o valor que o script imprime; o teste surface_normal_map_mean_matches_shader (eigc_terrain)
falha se a constante ficar desatualizada.
