use crate::config::{AnalysisConfig, AnalysisType, Environment, WindowUnit};
use crate::audio_io;
use crate::dsp;
use crate::utils;

use ndarray::{concatenate, Array, Array1, Array2, ArrayView2, Axis, s};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::Instant;
use chrono::NaiveDateTime;

// Helper struct to hold intermediate results for a single file
#[derive(Debug)] // Added Debug for easier inspection if needed
struct FileAnalysisResult {
    data: Array2<f64>, // [time/freq_header, values...]
    start_time: Option<NaiveDateTime>,
    // duration_secs: f64, // Can be calculated from data if needed
}

/// Processes a single audio file based on the configuration.
pub fn process_single_file( // Already pub, no change needed here
    file_path: &Path,
    config: &AnalysisConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing file: {}", file_path.display());
    let start_time = Instant::now();

    let (audio_data, fs_hz) = audio_io::read_wav_file(file_path)?;
    let fs = fs_hz as f64;
    println!("  Read {} samples at {} Hz", audio_data.len(), fs);

    let sensitivity_db = utils::calculate_system_sensitivity_db(config)?;
    println!("  System Sensitivity (S): {:.2} dB", sensitivity_db);

    let result = run_core_analysis(&audio_data, fs, config, sensitivity_db, None)?;

    if config.write_csv {
        let output_filename = generate_output_filename(file_path, config);
        let output_path = PathBuf::from(&config.output_dir).join(output_filename);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_csv(&output_path, &result.data)?;
        println!("  Output written to: {}", output_path.display());
    }

    let duration = start_time.elapsed();
    println!("  Finished processing in {:.2} seconds.", duration.as_secs_f64());
    Ok(())
}

/// Processes all audio files in a directory based on the configuration.
pub fn process_directory( // Already pub, no change needed here
    dir_path: &Path,
    config: &AnalysisConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing directory (batch mode): {}", dir_path.display());
    let overall_start_time = Instant::now();
    let mut file_results: Vec<FileAnalysisResult> = Vec::new();
    let mut processed_files_count = 0;

    fs::create_dir_all(&config.output_dir)?;

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "wav") {
            processed_files_count += 1;
            println!("Processing file {}: {}", processed_files_count, path.display());
            let file_start_time = Instant::now();

            let file_result = process_single_wav_for_batch(&path, config);

            match file_result {
                Ok(result) => {
                    // Optionally write individual CSV
                    if config.write_individual_batch_csvs && config.write_csv {
                        let output_filename = generate_output_filename(&path, config);
                        let output_path = PathBuf::from(&config.output_dir).join(output_filename);
                        match write_csv(&output_path, &result.data) {
                            Ok(_) => println!("  Individual output written to: {}", output_path.display()),
                            Err(e) => eprintln!("  Error writing individual CSV {}: {}", output_path.display(), e),
                        }
                    }
                    file_results.push(result);
                }
                Err(e) => {
                    eprintln!("  Error processing {}: {}. Skipping.", path.display(), e);
                }
            }

            let file_duration = file_start_time.elapsed();
            println!("  Finished processing {} in {:.2} seconds.", path.display(), file_duration.as_secs_f64());
        }
    }

    // Concatenate results if needed
    if config.create_batch_summary_file && !file_results.is_empty() && config.write_csv {
        println!("Concatenating results...");
        // Sort results by start time if timestamps were available and parsed
        if file_results.iter().all(|r| r.start_time.is_some()) {
            file_results.sort_by_key(|r| r.start_time.unwrap());
            println!("  Sorted files by timestamp.");
        } else {
            println!("  Warning: Not all files had parseable timestamps. Concatenating in directory order.");
            // TODO: Optionally implement offset time calculation if timestamps are missing
        }

        // Combine data arrays
        let first_result = &file_results[0];
        let header_row_view = first_result.data.slice(s![0..1, ..]); // Shape [1, N]
        let mut all_data_rows_views: Vec<ArrayView2<f64>> = Vec::with_capacity(file_results.len());

        for result in &file_results {
            if config.analysis_type == AnalysisType::Psd && result.data.ncols() != first_result.data.ncols() {
                eprintln!("  Error: Mismatched frequency bins between files ({} vs {} cols). Cannot concatenate PSD results.", result.data.ncols(), first_result.data.ncols());
                return Ok(());
            }
            all_data_rows_views.push(result.data.slice(s![1.., ..])); // Shape [M_i, N]
        }

        // Concatenate all data rows vertically
        let combined_data: Array2<f64> = concatenate(Axis(0), &all_data_rows_views)?;

        // Manually construct the final array by combining header and data
        let num_rows = combined_data.nrows() + 1;
        let num_cols = combined_data.ncols();
        let mut final_array = Array2::<f64>::zeros((num_rows, num_cols));
        final_array.slice_mut(s![0..1, ..]).assign(&header_row_view);
        final_array.slice_mut(s![1.., ..]).assign(&combined_data);

        // Write summary file
        let summary_filename = format!(
            "PAMGuide_Batch_{}_{:.0}Hz-{:.0}Hz_{}_Summary.csv", // Added cutoff frequencies
            match config.analysis_type { AnalysisType::Psd => "PSD", AnalysisType::Broadband => "Broadband" },
            config.low_cutoff,
            config.high_cutoff,
            if config.calibrated { "Calibrated" } else { "Relative" }
        );
        let summary_path = PathBuf::from(&config.output_dir).join(summary_filename);
        match write_csv(&summary_path, &final_array) {
            Ok(_) => println!("  Batch summary written to: {}", summary_path.display()),
            Err(e) => eprintln!("  Error writing batch summary CSV {}: {}", summary_path.display(), e),
        }

    } else if file_results.is_empty() {
        println!("No compatible audio files found in the directory.");
    }

    let overall_duration = overall_start_time.elapsed();
    println!("Batch processing finished in {:.2} seconds. Processed {} files.", overall_duration.as_secs_f64(), processed_files_count);
    Ok(())
}

/// Helper function to process a single WAV file for batch mode, returning the result struct.
fn process_single_wav_for_batch(
    path: &Path,
    config: &AnalysisConfig,
) -> Result<FileAnalysisResult, Box<dyn std::error::Error>> {
    let (audio_data, fs_hz) = audio_io::read_wav_file(path)?;
    let fs = fs_hz as f64;
    let sensitivity_db = utils::calculate_system_sensitivity_db(config)?;

    let file_start_datetime = if let Some(format) = &config.timestamp_format {
        parse_timestamp_from_filename(path, format)
    } else {
        None
    };
    if config.timestamp_format.is_some() && file_start_datetime.is_none() {
        eprintln!("  Warning: Could not parse timestamp from filename: {}. Time column will be relative for this file in summary.", path.display());
    }

    run_core_analysis(&audio_data, fs, config, sensitivity_db, file_start_datetime)
}


/// Core analysis function performing segmentation, FFT, and level calculation.
fn run_core_analysis(
    audio_data: &[f32],
    fs: f64,
    config: &AnalysisConfig,
    sensitivity_db: f64,
    file_start_time: Option<NaiveDateTime>,
) -> Result<FileAnalysisResult, Box<dyn std::error::Error>> {

    let n_total_samples = audio_data.len();
    let overlap_ratio = config.overlap_percentage / 100.0;

    let n_window_samples = match config.window_unit {
        WindowUnit::Samples => config.window_length as usize,
        WindowUnit::Seconds => (config.window_length * fs).round() as usize,
    };

    if n_window_samples == 0 || n_window_samples > n_total_samples {
        return Err(format!("Invalid window length {} for signal length {}", n_window_samples, n_total_samples).into());
    }

    let n_step = (n_window_samples as f64 * (1.0 - overlap_ratio)).round() as usize;
    if n_step == 0 {
         return Err("Overlap results in zero step size.".into());
    }

    let (scaled_window, _alpha) = dsp::generate_scaled_window(&config.window_type, n_window_samples);
    let noise_bw = dsp::noise_power_bandwidth(scaled_window.view(), n_window_samples);
    let delf = fs / n_window_samples as f64;

    let pref = match config.environment {
        Environment::Air => 20.0,
        Environment::Wat => 1.0,
    };

    // Calculate frequency axis and indices for slicing
    let fft_freqs: Array1<f64> = Array::linspace(0.0, fs / 2.0, n_window_samples / 2 + 1);
    // Note: MATLAB Pss goes from index 2 to N/2+1. Our fft_freqs maps to FFT result indices 0 to N/2.
    // Pss corresponds to indices 1 to N/2 of the full FFT result.
    // So, fft_freqs[1] corresponds to Pss[0].
    // We need indices relative to the Pss vector (length N/2).
    let pss_freqs = fft_freqs.slice(s![1..]); // Frequencies corresponding to Pss

    let pss_flow_idx = pss_freqs.iter().position(|&f| f >= config.low_cutoff).unwrap_or(0);
    let pss_fhigh_idx = pss_freqs.iter().rposition(|&f| f <= config.high_cutoff).unwrap_or(pss_freqs.len() - 1);

    if pss_flow_idx > pss_fhigh_idx {
         return Err(format!("Low cutoff {} Hz is >= high cutoff {} Hz after mapping to FFT bins.", config.low_cutoff, config.high_cutoff).into());
    }
    let selected_freqs = pss_freqs.slice(s![pss_flow_idx..=pss_fhigh_idx]);
    let n_selected_freqs = selected_freqs.len();

    // --- Segmentation and Parallel Processing ---
    let num_segments = if n_total_samples >= n_window_samples {
                           (n_total_samples - n_window_samples) / n_step + 1
                       } else { 0 };
    if num_segments == 0 {
        return Err("Audio signal too short for specified window length and overlap.".into());
    }

    let results_power: Vec<Vec<f64>> = (0..num_segments)
        .into_par_iter()
        .map(|i| {
            let start = i * n_step;
            let end = start + n_window_samples;
            let segment = &audio_data[start..end];

            let mut windowed_segment = segment.to_vec();
            for (sample, &win_val) in windowed_segment.iter_mut().zip(scaled_window.iter()) {
                *sample *= win_val;
            }

            let fft_result = dsp::calculate_fft(&windowed_segment);

            // Calculate single-sided power spectrum (linear) Pss
            let power_spectrum: Vec<f64> = fft_result[1..=n_window_samples / 2]
                .iter()
                .map(|c| (c.norm_sqr() / (n_window_samples as f32).powi(2)) as f64 * 2.0)
                .collect();

            // Select frequency range relative to Pss
            power_spectrum[pss_flow_idx..=pss_fhigh_idx].to_vec()
        })
        .collect();

     // --- Welch Averaging ---
     let (averaged_results, final_num_segments) = if let Some(welch_k) = config.welch_factor {
         if welch_k > 1 && welch_k <= num_segments {
             println!("  Applying Welch averaging with factor {}", welch_k);
             let num_welch_segments = (num_segments as f64 / welch_k as f64).ceil() as usize;
             let mut welch_averaged: Vec<Vec<f64>> = Vec::with_capacity(num_welch_segments);

             for i in 0..num_welch_segments {
                 let welch_start_idx = i * welch_k;
                 let welch_end_idx = std::cmp::min(welch_start_idx + welch_k, num_segments);
                 let segments_to_average = &results_power[welch_start_idx..welch_end_idx];

                 if !segments_to_average.is_empty() {
                     let mut avg_power = vec![0.0; n_selected_freqs];
                     for freq_idx in 0..n_selected_freqs {
                         let sum: f64 = segments_to_average.iter().map(|seg| seg[freq_idx]).sum();
                         avg_power[freq_idx] = sum / segments_to_average.len() as f64;
                     }
                     welch_averaged.push(avg_power);
                 }
             }
             (welch_averaged, num_welch_segments)
         } else {
             (results_power, num_segments)
         }
     } else {
         (results_power, num_segments)
     };

    // --- Convert to dB and Apply Calibration ---
    let mut final_results_db: Vec<Vec<f64>> = Vec::with_capacity(final_num_segments);
    for power_vec in averaged_results {
        let db_vec: Vec<f64> = match config.analysis_type {
            AnalysisType::Psd => {
                power_vec.iter()
                    .map(|&p| utils::power_to_db(p / (delf * noise_bw), pref) - sensitivity_db)
                    .collect()
            }
            AnalysisType::Broadband => {
                 let sum_power: f64 = power_vec.iter().sum();
                 vec![utils::power_to_db(sum_power, pref) - sensitivity_db] //-58.77
            }
        };
        final_results_db.push(db_vec);
    }

    // --- Construct Final Output Array ---
    let n_output_cols = match config.analysis_type {
        AnalysisType::Psd => n_selected_freqs,
        AnalysisType::Broadband => 1,
    };

    // Create header row (frequencies for PSD, 0.0 placeholder for Broadband time column)
    let mut header_row_vec = vec![0.0; n_output_cols + 1]; // +1 for time column
    if config.analysis_type == AnalysisType::Psd {
         header_row_vec[1..].copy_from_slice(selected_freqs.as_slice().unwrap());
    }
    let header_row = Array1::from(header_row_vec);

    // Create data rows
    let mut data_rows = Array2::<f64>::zeros((final_num_segments, n_output_cols + 1));
    let time_step_secs = n_step as f64 / fs;
    let welch_time_multiplier = config.welch_factor.unwrap_or(1) as f64;

    for (i, db_vec) in final_results_db.iter().enumerate() {
        let time_secs = i as f64 * time_step_secs * welch_time_multiplier;
        let current_time = if let Some(start_dt) = file_start_time {
            let start_secs = start_dt.timestamp() as f64 + start_dt.timestamp_subsec_nanos() as f64 * 1e-9;
            start_secs + time_secs
        } else {
            time_secs
        };
        data_rows[[i, 0]] = current_time;
        for (j, &db_val) in db_vec.iter().enumerate() {
            data_rows[[i, j + 1]] = db_val;
        }
    }

    // Combine header and data manually
    let num_rows = data_rows.nrows() + 1;
    let num_cols = data_rows.ncols();
    let mut final_array = Array2::<f64>::zeros((num_rows, num_cols));
    final_array.row_mut(0).assign(&header_row);
    final_array.slice_mut(s![1.., ..]).assign(&data_rows);

    Ok(FileAnalysisResult {
        data: final_array,
        start_time: file_start_time,
        // duration_secs: total_duration_secs, // Removed, can be inferred
    })
}

/// Generates the output CSV filename based on input path and config.
fn generate_output_filename(input_path: &Path, config: &AnalysisConfig) -> String {
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let analysis_str = match config.analysis_type {
        AnalysisType::Psd => "PSD",
        AnalysisType::Broadband => "Broadband",
    };
    let window_len_str = match config.window_unit {
         WindowUnit::Seconds => format!("{:.2}s", config.window_length),
         WindowUnit::Samples => format!("{}samples", config.window_length as usize),
    };
    let window_name_str = format!("{:?}", config.window_type);

    format!(
        "{}_{}_{}{}_{:.0}PercentOverlap.csv",
        stem,
        analysis_str,
        window_len_str,
        window_name_str,
        config.overlap_percentage
    )
}

/// Writes the analysis data array to a CSV file.
fn write_csv(path: &Path, data: &Array2<f64>) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::create(path)?;
    let mut wtr = csv::WriterBuilder::new().has_headers(false).from_writer(file);

    // Write header row
    let header_iter = data.row(0).into_iter().map(|&f| {
        // Leave time column header blank, format frequencies
        if f == 0.0 { "".to_string() } else { format!("{:.4}", f) }
    });
    wtr.write_record(header_iter)?;

    // Write data rows
    for row in data.rows().into_iter().skip(1) {
        let row_iter = row.into_iter().enumerate().map(|(i, &val)| {
            if i == 0 { // Time column
                if val > 1e9 { // Heuristic for Unix timestamp
                    match NaiveDateTime::from_timestamp_opt(val as i64, (val.fract() * 1e9) as u32) {
                        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                        None => format!("{:.3}", val), // Fallback
                    }
                } else {
                    format!("{:.3}", val) // Relative time
                }
            } else { // Data columns
                format!("{:.4}", val)
            }
        });
        wtr.write_record(row_iter)?;
    }

    wtr.flush()?;
    Ok(())
}

/// Parses timestamp from filename using the provided format string.
/// Expects format like "PREFIX.YYYYMMDDTHHMMSSZ.wav"
fn parse_timestamp_from_filename(path: &Path, format: &str) -> Option<NaiveDateTime> {
    let stem = path.file_stem()?.to_string_lossy();
    // Extract the part after the first '.' and before the second '.' (or end of stem)
    let parts: Vec<&str> = stem.splitn(2, '.').collect();
    if parts.len() == 2 {
        let timestamp_str = parts[1];
         // Optional: Remove trailing 'Z' if present and format doesn't handle it
         // let timestamp_str_trimmed = timestamp_str.strip_suffix('Z').unwrap_or(timestamp_str);

        match NaiveDateTime::parse_from_str(timestamp_str, format) {
            Ok(dt) => Some(dt),
            Err(e) => {
                eprintln!("  Timestamp parse error for '{}' (extracted: '{}') with format '{}': {}", stem, timestamp_str, format, e);
                None
            }
        }
    } else {
        eprintln!("  Could not extract timestamp part from filename stem: {}", stem);
        None
    }
}

// Helper to convert PAMGuide format (y, m, d, H, M, S, F) to chrono format
// fn convert_pamguide_format_to_chrono(pg_format: &str) -> String { ... } // TODO
