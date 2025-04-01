use crate::config::{AnalysisConfig, CalibrationType, Environment};

/// Calculates the overall system sensitivity correction factor (S) in dB.
/// Returns 0.0 if calibration is not enabled.
pub fn calculate_system_sensitivity_db(config: &AnalysisConfig) -> Result<f64, String> {
    if !config.calibrated {
        return Ok(0.0);
    }

    // Ensure calibration type is specified if calibrated is true
    let cal_type = config.calibration_type.as_ref()
        .ok_or_else(|| "Calibration type must be specified when calibrated=true".to_string())?;

    // Reference pressure adjustment for air vs water
    // MATLAB code converts Air Mh to dB re 1 V/uPa by subtracting 120 dB.
    // We'll apply this adjustment directly to Mh if needed.
    let mut mh_adjusted = config.mic_hydro_sensitivity;
    if let Some(mh) = mh_adjusted {
        if config.environment == Environment::Air {
            mh_adjusted = Some(mh - 120.0); // Convert dB re 1 V/Pa to dB re 1 V/uPa
        }
    }


    match cal_type {
        CalibrationType::Ts => {
            // S = Mh + G + 20*log10(1/vADC)
            let mh = mh_adjusted.ok_or_else(|| "mic_hydro_sensitivity is required for TS calibration".to_string())?;
            let g = config.preamp_gain.ok_or_else(|| "preamp_gain is required for TS calibration".to_string())?;
            let vadc = config.adc_vpeak.ok_or_else(|| "adc_vpeak is required for TS calibration".to_string())?;
            if vadc <= 0.0 {
                return Err("adc_vpeak must be positive".to_string());
            }
            // The 20*log10(1/vADC) term is handled by MATLAB's normalization.
            // Since we normalize manually in read_wav_file, we don't need this term here.
            // However, the original PAMGuide paper Appendix S1 Eq 4 includes it.
            // Let's follow the MATLAB code's apparent implementation which omits it.
            // S = Mh + G
             Ok(mh + g)
            // If following paper strictly: Ok(mh + g + 20.0 * (1.0 / vadc).log10())

        }
        CalibrationType::Ee => {
            // S = Si (End-to-end sensitivity)
            config.system_sensitivity.ok_or_else(|| "system_sensitivity is required for EE calibration".to_string())
        }
        CalibrationType::Rc => {
            // S = Si + Mh (Recorder sensitivity + Mic/Hydro sensitivity)
             let si = config.system_sensitivity.ok_or_else(|| "system_sensitivity is required for RC calibration".to_string())?;
             let mh = mh_adjusted.ok_or_else(|| "mic_hydro_sensitivity is required for RC calibration".to_string())?;
             Ok(si + mh)
        }
    }
}

/// Converts a linear power value to decibels relative to a reference.
#[inline]
pub fn power_to_db(value: f64, reference: f64) -> f64 {
    if value <= 0.0 || reference <= 0.0 {
        // Handle non-positive values, e.g., return -Infinity or a very small number
        // Or based on how MATLAB handles log10(0) or log10(negative)
        f64::NEG_INFINITY // Or consider returning NaN or a specific error
    } else {
        10.0 * (value / reference.powi(2)).log10()
    }
}

