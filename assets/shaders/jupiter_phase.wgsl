// Iluminação própria do disco de Júpiter no céu.
//
// Júpiter recebe só a luz do sol, calculada aqui a partir da direção do sol (uniform), e não as
// luzes da cena: a luz ambiente acenderia o lado noturno, e a `SunLight` se apaga quando o sol
// passa abaixo do horizonte do recorte, mas o sol continua iluminando Júpiter.
//
// A conta repete a parte difusa da luz direcional do Bevy (exposição x cor difusa x iluminância
// x N.L / pi), para o lado iluminado ficar com o mesmo brilho que tinha com o `StandardMaterial`.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
}

// xyz: direção para o sol, no espaço do mundo. w: iluminância do sol, em lux.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> sun: vec4<f32>;

const PI: f32 = 3.141592653589793;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let pbr_input = pbr_input_from_standard_material(in, is_front);

    let n_dot_l = max(dot(normalize(pbr_input.N), normalize(sun.xyz)), 0.0);
    let lit = pbr_input.material.base_color.rgb * sun.w * n_dot_l * view.exposure / PI;

    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr_input, vec4<f32>(lit, 1.0));
    return out;
}
