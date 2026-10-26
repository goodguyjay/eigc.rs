// Mistura de duas escalas do material do terreno.
//
// O StandardMaterial base já amostra as texturas na escala curta (UV = metros / tile físico).
// Aqui o mesmo conjunto de texturas é amostrado de novo numa escala `razão` vezes maior e a
// mistura entre as duas escalas é feita pela distância à câmera, de perto vale a escala curta
// (detalhe fino), de longe a escala longa (feições grandes o bastante para sobreviver ao mip).

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

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_UVS_A
    if detail_params.w > 0.5 {
        let distance_to_camera = distance(in.world_position.xyz, view.world_position);
        let far_weight = smoothstep(detail_params.y, detail_params.z, distance_to_camera);

        // As amostras ficam fora de qualquer `if` de propósito: derivadas (mip) exigem fluxo
        // de controle uniforme.
        let macro_uv = in.uv / detail_params.x;
        let macro_diffuse = textureSampleBias(
            macro_diffuse_texture, macro_diffuse_sampler, macro_uv, view.mip_bias);
        let macro_normal = textureSampleBias(
            macro_normal_texture, macro_normal_sampler, macro_uv, view.mip_bias);
        let macro_arm = textureSampleBias(
            macro_arm_texture, macro_arm_sampler, macro_uv, view.mip_bias);

#ifdef VERTEX_COLORS
        let vertex_tint = in.color;
#else
        let vertex_tint = vec4<f32>(1.0);
#endif
        pbr_input.material.base_color =
            mix(pbr_input.material.base_color, vertex_tint * macro_diffuse, far_weight);
        pbr_input.material.perceptual_roughness =
            mix(pbr_input.material.perceptual_roughness, macro_arm.g, far_weight);
        pbr_input.material.metallic = mix(pbr_input.material.metallic, macro_arm.b, far_weight);
        pbr_input.diffuse_occlusion =
            mix(pbr_input.diffuse_occlusion, vec3<f32>(macro_arm.r), far_weight);

#ifdef VERTEX_TANGENTS
        let tbn = calculate_tbn_mikktspace(in.world_normal, in.world_tangent);
        let macro_n = apply_normal_mapping(
            pbr_input.material.flags, tbn, false, is_front, macro_normal.rgb);
        pbr_input.N = normalize(mix(pbr_input.N, macro_n, far_weight));
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
