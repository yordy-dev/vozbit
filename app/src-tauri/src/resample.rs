use std::f64::consts::PI;

/// Offline low-pass conversion to Whisper's required 16 kHz mono PCM.
/// The capture callback never runs this work.
pub fn to_16khz(input: &[f32], input_rate: u32) -> Result<Vec<f32>, String> {
    const OUTPUT_RATE: u32 = 16_000;
    if input_rate == 0 {
        return Err("La frecuencia de entrada no puede ser cero".into());
    }
    if input_rate == OUTPUT_RATE {
        return Ok(input.to_vec());
    }
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let output_len =
        ((input.len() as f64 * OUTPUT_RATE as f64 / input_rate as f64).round() as usize).max(1);
    let ratio = input_rate as f64 / OUTPUT_RATE as f64;
    let cutoff = (OUTPUT_RATE as f64 / input_rate as f64).min(1.0) * 0.92;
    let radius = 24_i64;
    let mut output = Vec::with_capacity(output_len);

    for out_index in 0..output_len {
        let center = out_index as f64 * ratio;
        let base = center.floor() as i64;
        let mut sum = 0.0_f64;
        let mut weight_sum = 0.0_f64;
        for index in (base - radius + 1)..=(base + radius) {
            if index < 0 || index >= input.len() as i64 {
                continue;
            }
            let distance = center - index as f64;
            let x = distance / radius as f64;
            if x.abs() >= 1.0 {
                continue;
            }
            let window = 0.42 + 0.5 * (PI * x).cos() + 0.08 * (2.0 * PI * x).cos();
            let phase = PI * distance * cutoff;
            let sinc = if phase.abs() < 1e-10 {
                1.0
            } else {
                phase.sin() / phase
            };
            let weight = cutoff * sinc * window;
            sum += input[index as usize] as f64 * weight;
            weight_sum += weight;
        }
        output.push(if weight_sum.abs() > 1e-12 {
            (sum / weight_sum) as f32
        } else {
            0.0
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::to_16khz;
    use std::f32::consts::PI;

    fn tone(rate: u32, frequency: f32, seconds: usize) -> Vec<f32> {
        (0..rate as usize * seconds)
            .map(|n| (2.0 * PI * frequency * n as f32 / rate as f32).sin())
            .collect()
    }

    fn rms(values: &[f32]) -> f32 {
        (values.iter().map(|x| x * x).sum::<f32>() / values.len() as f32).sqrt()
    }

    #[test]
    fn converts_common_hardware_rates_to_exact_duration() {
        for rate in [44_100, 48_000] {
            let output = to_16khz(&tone(rate, 1_000.0, 2), rate).unwrap();
            assert_eq!(output.len(), 32_000);
            assert!((rms(&output[200..31_800]) - 0.707).abs() < 0.03);
        }
    }

    #[test]
    fn attenuates_frequencies_above_new_nyquist_limit() {
        let output = to_16khz(&tone(48_000, 12_000.0, 1), 48_000).unwrap();
        assert!(rms(&output[200..15_800]) < 0.08);
    }

    #[test]
    fn preserves_native_rate_and_handles_empty_input() {
        assert_eq!(to_16khz(&[0.25, -0.25], 16_000).unwrap(), [0.25, -0.25]);
        assert!(to_16khz(&[], 48_000).unwrap().is_empty());
        assert!(to_16khz(&[1.0], 0).is_err());
    }
}
