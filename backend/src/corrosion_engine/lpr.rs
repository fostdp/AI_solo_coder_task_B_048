const B: f64 = 0.026;
const IRON_DENSITY: f64 = 7.87;
const IRON_ATOMIC_WEIGHT: f64 = 55.85;
const IRON_VALENCE: f64 = 2.0;
const COPPER_DENSITY: f64 = 8.96;
const COPPER_ATOMIC_WEIGHT: f64 = 63.55;
const COPPER_VALENCE: f64 = 2.0;
const FARADAY: f64 = 96485.0;
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

pub fn calculate_corrosion_rate_lpr(polarization_resistance: f64, material_type: &str) -> f64 {
    let rp = polarization_resistance.max(10.0);
    let icorr = B / rp;

    let (atomic_weight, valence, density) = match material_type {
        "iron" | "Iron" | "铁" => (IRON_ATOMIC_WEIGHT, IRON_VALENCE, IRON_DENSITY),
        "copper" | "Copper" | "铜" => (COPPER_ATOMIC_WEIGHT, COPPER_VALENCE, COPPER_DENSITY),
        _ => (IRON_ATOMIC_WEIGHT, IRON_VALENCE, IRON_DENSITY),
    };

    let rate = (3.27e-3 * icorr * atomic_weight * SECONDS_PER_YEAR)
        / (valence * FARADAY * density * 1e-3);

    rate.max(0.0001)
}
