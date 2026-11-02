// Mistura de duas escalas do material do terreno, com amostragem anti-tiling (hex tiling).
//
// As mesmas texturas são amostradas em duas escalas: a curta (UV = metros / tile físico) e uma
// `razão` vezes maior. A mistura entre as duas é feita pela distância à câmera, de perto vale a
// escala curta (detalhe fino), de longe a escala longa (feições grandes o bastante para
// sobreviver ao mip).
//
// As duas escalas são amostradas aqui, e não pelo StandardMaterial, para poderem quebrar a
// repetição: cada amostra é a mistura de três cópias da textura deslocadas por um offset
// aleatório por vértice de uma grade triangular (Mikkelsen, "Practical Real-Time Hex-Tiling").
// Isso depende de os escalares do StandardMaterial estarem neutros (branco, roughness 1.0), o que
// `terrain_material_neutral_scalars` garante.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{
        alpha_discard,
        apply_normal_mapping,
        apply_pbr_lighting,
        calculate_tbn_mikktspace,
        main_pass_post_lighting_processing,
    },
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
}

// x: razão entre a escala longa e a curta, y: distância onde a mistura começa (m),
// z: distância onde a escala longa vale 100% (m), w: 1.0 se a camada longa está habilitada.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> detail_params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var macro_diffuse_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var macro_diffuse_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var macro_normal_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var macro_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var macro_arm_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var macro_arm_sampler: sampler;

// Lado do triângulo da grade, em tiles. Maior = menos regiões de transição (e menos mistura
// fantasma), mas a repetição de cada trecho dura mais.
const HEX_CELL_SCALE: f32 = 1.0;

// Expoente aplicado aos pesos dos três vértices antes de normalizar. Maior = transições mais
// curtas (menos ghosting, emenda mais visível); 1.0 seria a mistura linear.
const HEX_BLEND_SHARPNESS: f32 = 6.0;

// Três offsets de UV aleatórios (um por vértice do triângulo que contém o ponto) e os pesos
// de mistura correspondentes.
struct HexTile {
    weights: vec3<f32>,
    offset_a: vec2<f32>,
    offset_b: vec2<f32>,
    offset_c: vec2<f32>,
}

// Hash PCG de 2 componentes sobre coordenadas inteiras, devolve dois valores em [0, 1].
fn hash_cell(cell: vec2<i32>) -> vec2<f32> {
    var v = vec2<u32>(bitcast<u32>(cell.x), bitcast<u32>(cell.y)) * 1664525u
        + vec2<u32>(1013904223u);
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    return vec2<f32>(v) * (1.0 / 4294967295.0);
}

fn hex_tile(uv: vec2<f32>) -> HexTile {
    let scaled = uv * HEX_CELL_SCALE;
    // Inclina o espaço para que a grade triangular equilátera vire uma grade de quadrados.
    let skewed = vec2<f32>(scaled.x, -0.57735027 * scaled.x + 1.15470054 * scaled.y);
    let base_id = vec2<i32>(floor(skewed));
    let f = fract(skewed);

    // Qual dos dois triângulos do quadrado contém o ponto, e os pesos baricêntricos nele.
    let z = 1.0 - f.x - f.y;
    let s = step(0.0, -z);
    let orientation = 2.0 * s - 1.0;
    var weights = vec3<f32>(-z * orientation, s - f.y * orientation, s - f.x * orientation);
    let si = i32(s);

    weights = pow(max(weights, vec3<f32>(0.0)), vec3<f32>(HEX_BLEND_SHARPNESS));
    weights = weights / (weights.x + weights.y + weights.z);

    return HexTile(
        weights,
        hash_cell(base_id + vec2<i32>(si, si)),
        hash_cell(base_id + vec2<i32>(si, 1 - si)),
        hash_cell(base_id + vec2<i32>(1 - si, si)),
    );
}

// Amostra a textura nos três offsets do `tile` e mistura. Usa gradiente explícito (o da UV sem
// offset) para o mip não saltar nas emendas entre triângulos.
fn sample_hex(
    tex: texture_2d<f32>,
    samp: sampler,
    tile: HexTile,
    uv: vec2<f32>,
    dx: vec2<f32>,
    dy: vec2<f32>,
) -> vec4<f32> {
    return tile.weights.x * textureSampleGrad(tex, samp, uv + tile.offset_a, dx, dy)
        + tile.weights.y * textureSampleGrad(tex, samp, uv + tile.offset_b, dx, dy)
        + tile.weights.z * textureSampleGrad(tex, samp, uv + tile.offset_c, dx, dy);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_UVS_A
    if detail_params.w > 0.5 {
        let distance_to_camera = distance(in.world_position.xyz, view.world_position);
        let far_weight = smoothstep(detail_params.y, detail_params.z, distance_to_camera);

        // Derivadas (mip) exigem fluxo de controle uniforme, por isso saem antes dos `if`
        // abaixo, que dependem da distância.
        let gradient_scale = exp2(view.mip_bias);
        let near_dx = dpdx(in.uv) * gradient_scale;
        let near_dy = dpdy(in.uv) * gradient_scale;
        let macro_uv = in.uv / detail_params.x;
        let macro_dx = near_dx / detail_params.x;
        let macro_dy = near_dy / detail_params.x;

        var diffuse = vec4<f32>(0.0);
        var normal = vec4<f32>(0.0);
        var arm = vec4<f32>(0.0);

        // Só paga as amostras da escala que de fato contribui.
        if far_weight < 1.0 {
            let tile = hex_tile(in.uv);
            let near_weight = 1.0 - far_weight;
            diffuse += near_weight * sample_hex(
                macro_diffuse_texture, macro_diffuse_sampler, tile, in.uv, near_dx, near_dy);
            normal += near_weight * sample_hex(
                macro_normal_texture, macro_normal_sampler, tile, in.uv, near_dx, near_dy);
            arm += near_weight * sample_hex(
                macro_arm_texture, macro_arm_sampler, tile, in.uv, near_dx, near_dy);
        }
        if far_weight > 0.0 {
            let tile = hex_tile(macro_uv);
            diffuse += far_weight * sample_hex(
                macro_diffuse_texture, macro_diffuse_sampler, tile, macro_uv, macro_dx, macro_dy);
            normal += far_weight * sample_hex(
                macro_normal_texture, macro_normal_sampler, tile, macro_uv, macro_dx, macro_dy);
            arm += far_weight * sample_hex(
                macro_arm_texture, macro_arm_sampler, tile, macro_uv, macro_dx, macro_dy);
        }

#ifdef VERTEX_COLORS
        let vertex_tint = in.color;
#else
        let vertex_tint = vec4<f32>(1.0);
#endif
        pbr_input.material.base_color = vertex_tint * diffuse;
        pbr_input.material.perceptual_roughness = arm.g;
        pbr_input.material.metallic = mix(pbr_input.material.metallic, arm.b, far_weight);
        pbr_input.diffuse_occlusion = vec3<f32>(arm.r);

#ifdef VERTEX_TANGENTS
        let tbn = calculate_tbn_mikktspace(in.world_normal, in.world_tangent);
        pbr_input.N = normalize(apply_normal_mapping(
            pbr_input.material.flags, tbn, false, is_front, normal.rgb));
#endif
    }
#endif

    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
