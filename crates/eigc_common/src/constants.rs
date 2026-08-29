/// Ângulo do sol visto de ~5,2 AU em graus (~0.5° / 5,2 AU)
pub const SUN_ANGULAR_DIAMETER_DEG: f32 = 0.10;
/// Obliquidade real do eixo de Júpiter
pub const JUPITER_OBLIQUITY_DEG: f32 = 3.13;
/// Rotação de Júpiter em segundos (9h 55m 30s)
pub const JUPITER_ROTATION_PERIOD_SECS : f32 = 9.925 * 3600.0;
/// Velocidade padrão da câmera flycam
pub const FLY_CAM_DEFAULT_SPEED: f32 = 20.0;
/// Sensibilidade padrão da câmera flycam
pub const FLY_CAM_DEFAULT_SENSITIVITY: f32 = 0.002;
/// Multiplicador de velocidade da câmera flycam ao segurar Shift
pub const FLY_CAM_BOOST_MULTIPLIER: f32 = 10.0;
/// Limite de pitch da câmera flycam em radianos (aprox. 88°)
pub const FLY_CAM_PITCH_CLAMP_RAD: f32 = 1.54;
/// Distância mínima do plano de corte da câmera
pub const CAMERA_NEAR_PLANE: f32 = 0.1;
/// Distância máxima do plano de corte da câmera
pub const CAMERA_FAR_PLANE: f32 = 50_000.0;