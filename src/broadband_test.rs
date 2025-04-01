use crate::config::{AnalysisConfig, AnalysisType, Environment, WindowType, WindowUnit, load_config};
use crate::audio_io;
use crate::dsp;
use crate::utils;

use ndarray::{Array1, s};
use rustfft::num_complex::Complex;
use std::path::Path;

pub fn run_broadband_test(wav_file_path: &str, config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== BROADBAND ANALYSIS TEST ===");
    println!("This test will process a WAV file step by step and output intermediate values.");
    println!();

    // Load configuration
    let config_path = Path::new(config_path);
    let config = load_config(config_path)?;
    println!("Configuration loaded from: {}", config_path.display());
    println!("  Analysis type: {:?}", config.analysis_type);
    println!("  Environment: {:?}", config.environment);
    println!("  Calibration: {}", if config.calibrated { "Enabled" } else { "Disabled" });
    if config.calibrated {
        println!("  Calibration type: {:?}", config.calibration_type.as_ref().unwrap());
    }
    println!("  Window type: {:?}", config.window_type);
    println!("  Window length: {} {:?}", config.window_length, config.window_unit);
    println!("  Overlap: {}%", config.overlap_percentage);
    println!("  Frequency range: {} Hz to {} Hz", config.low_cutoff, config.high_cutoff);
    println!();

    // Read WAV file
    let wav_file_path = Path::new(wav_file_path);
    let (audio_data, fs_hz) = audio_io::read_wav_file(wav_file_path)?;
    let fs = fs_hz as f64;
    println!("WAV file read: {}", wav_file_path.display());
    println!("  Sample rate: {} Hz", fs);
    println!("  Number of samples: {}", audio_data.len());
    println!("  Duration: {:.2} seconds", audio_data.len() as f64 / fs);
    println!();

    // --- Diagnostic Output: Raw Audio Data Statistics ---
    let min = audio_data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = audio_data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let mean: f32 = audio_data.iter().sum::<f32>() / audio_data.len() as f32;
    let squared_sum: f32 = audio_data.iter().map(|&x| x * x).sum();
    let rms = (squared_sum / audio_data.len() as f32).sqrt();

    println!("RAW AUDIO DATA STATISTICS:");
    println!("  Min: {}", min);
    println!("  Max: {}", max);
    println!("  Mean: {}", mean);
    println!("  RMS: {}", rms);
    println!();

    // Calculate system sensitivity
    let sensitivity_db = utils::calculate_system_sensitivity_db(&config)?;
    println!("SYSTEM SENSITIVITY:");
    println!("  S = {:.2} dB", sensitivity_db);
    println!();

    // Determine window parameters
    let n_total_samples = audio_data.len();
    let overlap_ratio = config.overlap_percentage / 100.0;
    let n_window_samples = match config.window_unit {
        WindowUnit::Samples => config.window_length as usize,
        WindowUnit::Seconds => (config.window_length * fs).round() as usize,
    };
    
    println!("WINDOW PARAMETERS:");
    println!("  Window length: {} samples ({:.4} seconds)", n_window_samples, n_window_samples as f64 / fs);
    println!("  Overlap ratio: {:.2}", overlap_ratio);
    
    // Calculate number of segments
    let n_step = (n_window_samples as f64 * (1.0 - overlap_ratio)).round() as usize;
    let num_segments = if n_total_samples >= n_window_samples {
        (n_total_samples - n_window_samples) / n_step + 1
    } else { 0 };
    
    println!("  Step size: {} samples", n_step);
    println!("  Number of segments: {}", num_segments);
    println!();

    // Generate window function
    let (scaled_window, alpha) = dsp::generate_scaled_window(&config.window_type, n_window_samples);
    let noise_bw = dsp::noise_power_bandwidth(scaled_window.view(), n_window_samples);
    
    println!("WINDOW FUNCTION:");
    println!("  Window type: {:?}", config.window_type);
    println!("  Alpha (scaling factor): {:.4}", alpha);
    println!("  Noise power bandwidth (B): {:.6}", noise_bw);
    
    // Print first few window values
    println!("  First 5 window values:");
    for i in 0..5.min(scaled_window.len()) {
        println!("    w[{}] = {:.6}", i, scaled_window[i]);
    }
    println!();

    // Calculate frequency parameters
    let delf = fs / n_window_samples as f64;
    let fft_freqs: Array1<f64> = Array1::linspace(0.0, fs / 2.0, n_window_samples / 2 + 1);
    let pss_freqs = fft_freqs.slice(s![1..]);
    
    let pss_flow_idx = pss_freqs.iter().position(|&f| f >= config.low_cutoff).unwrap_or(0);
    let pss_fhigh_idx = pss_freqs.iter().rposition(|&f| f <= config.high_cutoff).unwrap_or(pss_freqs.len() - 1);
    let selected_freqs = pss_freqs.slice(s![pss_flow_idx..=pss_fhigh_idx]);
    
    println!("FREQUENCY PARAMETERS:");
    println!("  Frequency bin width (delf): {:.4} Hz", delf);
    println!("  Number of frequency bins: {}", fft_freqs.len());
    println!("  Low cutoff index: {} ({:.2} Hz)", pss_flow_idx, pss_freqs[pss_flow_idx]);
    println!("  High cutoff index: {} ({:.2} Hz)", pss_fhigh_idx, pss_freqs[pss_fhigh_idx]);
    println!("  Number of selected frequency bins: {}", selected_freqs.len());
    println!();

    // Reference pressure
    let pref = match config.environment {
        Environment::Air => 20e-6,
        Environment::Wat => 1e-6,
    };
    println!("REFERENCE PRESSURE:");
    println!("  pref = {:.6e} Pa", pref);
    println!();

    // Process first segment as an example
    println!("PROCESSING FIRST SEGMENT AS EXAMPLE:");
    
    // Extract first segment
    let segment = &audio_data[0..n_window_samples];
    println!("  Segment length: {} samples", segment.len());
    
    // Apply window
    let mut windowed_segment = segment.to_vec();
    for (sample, &win_val) in windowed_segment.iter_mut().zip(scaled_window.iter()) {
        *sample *= win_val;
    }
    
    // Calculate statistics of windowed segment
    let win_min = windowed_segment.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let win_max = windowed_segment.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let win_mean: f32 = windowed_segment.iter().sum::<f32>() / windowed_segment.len() as f32;
    let win_squared_sum: f32 = windowed_segment.iter().map(|&x| x * x).sum();
    let win_rms = (win_squared_sum / windowed_segment.len() as f32).sqrt();
    
    println!("  Windowed segment statistics:");
    println!("    Min: {}", win_min);
    println!("    Max: {}", win_max);
    println!("    Mean: {}", win_mean);
    println!("    RMS: {}", win_rms);
    
    // Calculate FFT
    let fft_result = dsp::calculate_fft(&windowed_segment);
    
    println!("  FFT result (first 5 values):");
    for i in 0..5.min(fft_result.len()) {
        println!("    X[{}] = {:.6} + {:.6}i (|X| = {:.6})", 
                 i, fft_result[i].re, fft_result[i].im, fft_result[i].norm());
    }
    
    // Calculate power spectrum
    let power_spectrum: Vec<f64> = fft_result[1..=n_window_samples / 2]
        .iter()
        .map(|c| (c.norm_sqr() / (n_window_samples as f32).powi(2)) as f64 * 2.0)
        .collect();
    
    println!("  Power spectrum (first 5 values):");
    for i in 0..5.min(power_spectrum.len()) {
        println!("    P[{}] = {:.6e}", i, power_spectrum[i]);
    }
    
    // Select frequency range
    let selected_power = &power_spectrum[pss_flow_idx..=pss_fhigh_idx];
    
    println!("  Selected power spectrum (first 5 values):");
    for i in 0..5.min(selected_power.len()) {
        println!("    P_sel[{}] = {:.6e}", i, selected_power[i]);
    }
    
    // Calculate sum of power
    let sum_power: f64 = selected_power.iter().sum();
    println!("  Sum of power: {:.6e}", sum_power);
    
    // Convert to dB
    let db_without_constant = utils::power_to_db(sum_power, pref) - sensitivity_db;
    
    println!("  Broadband level:");
    println!("    Without constant: {:.2} dB", db_without_constant);
    println!();

    // Process all segments
    println!("PROCESSING ALL SEGMENTS:");
    
    let mut all_segment_powers: Vec<f64> = Vec::with_capacity(num_segments);
    
    for i in 0..num_segments {
        let start = i * n_step;
        let end = start + n_window_samples;
        let segment = &audio_data[start..end];
        
        let mut windowed_segment = segment.to_vec();
        for (sample, &win_val) in windowed_segment.iter_mut().zip(scaled_window.iter()) {
            *sample *= win_val;
        }
        
        let fft_result = dsp::calculate_fft(&windowed_segment);
        
        let power_spectrum: Vec<f64> = fft_result[1..=n_window_samples / 2]
            .iter()
            .map(|c| (c.norm_sqr() / (n_window_samples as f32).powi(2)) as f64 * 2.0)
            .collect();
        
        let selected_power = &power_spectrum[pss_flow_idx..=pss_fhigh_idx];
        let sum_power: f64 = selected_power.iter().sum();
        all_segment_powers.push(sum_power);
    }
    
    // Calculate statistics of all segment powers
    let min_power = all_segment_powers.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max_power = all_segment_powers.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let mean_power = all_segment_powers.iter().sum::<f64>() / all_segment_powers.len() as f64;
    
    println!("  All segment powers statistics:");
    println!("    Min: {:.6e}", min_power);
    println!("    Max: {:.6e}", max_power);
    println!("    Mean: {:.6e}", mean_power);
    
    // Convert mean to dB
    let mean_db_with_constant = utils::power_to_db(mean_power, pref) - sensitivity_db - 58.77;
    let mean_db_without_constant = utils::power_to_db(mean_power, pref) - sensitivity_db;
    
    println!("  Mean broadband level:");
    println!("    Without constant: {:.2} dB", mean_db_without_constant);
    println!("    With -58.77 constant: {:.2} dB", mean_db_with_constant);
    println!();

    println!("TEST COMPLETE");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadband() {
        let result = run_broadband_test("AMAR394.20240717T164721Z.wav", "pamguide_rust/config.toml");
        assert!(result.is_ok());
    }
}
