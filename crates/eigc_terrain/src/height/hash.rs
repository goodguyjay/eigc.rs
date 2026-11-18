//! Sorteio determinístico compartilhado pelas feições geradas por semente (paterae, crateras).

/// Valor pseudo-aleatório determinístico em [0, 1) a partir de uma semente e um índice
/// (mistura splitmix64).
pub(crate) fn hash01(seed: u32, index: u32) -> f32 {
    let mut z = (u64::from(seed) << 32 | u64::from(index)).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1_u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testa que o hash é determinístico, fica em [0, 1) e espalha os valores pela faixa.
    #[test]
    fn hash01_is_deterministic_and_spread_over_unit_interval() {
        let values: Vec<f32> = (0..1000).map(|i| hash01(7, i)).collect();
        assert_eq!(values, (0..1000).map(|i| hash01(7, i)).collect::<Vec<_>>());
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));

        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "média {mean}");
        assert!(values.iter().any(|&v| v < 0.05) && values.iter().any(|&v| v > 0.95));
        assert_ne!(hash01(7, 0), hash01(8, 0));
    }
}
