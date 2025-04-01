use serde::Deserialize;
use std::fs;
use std::path::Path;

// Define enums for configuration options with specific values
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisType {
    Psd,
    Broadband,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Air,
    Wat, // Water
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CalibrationType {
    Ts, // Transducer Specs
    Ee, // End-to-End
    Rc, // Recorder + Hydrophone/Microphone
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowType {
    Hann,
    Hamming,
    Blackman,
    Rectangular, // Equivalent to 'None' in MATLAB
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowUnit {
    Seconds,
    Samples,
}


// Main configuration struct mirroring the TOML file structure
#[derive(Deserialize, Debug, Clone)]
pub struct AnalysisConfig {
    // Input/Output Settings
    pub input_path: String,
    pub output_dir: String,
    #[serde(default = "default_true")]
    pub write_csv: bool,
    #[serde(default = "default_true")] // Default to creating summary unless specified otherwise
    pub create_batch_summary_file: bool,
    #[serde(default = "default_false")] // Default to not writing individual files in batch mode
    pub write_individual_batch_csvs: bool,

    // Core Analysis Settings
    pub analysis_type: AnalysisType,
    pub environment: Environment,

    // Calibration Settings
    #[serde(default = "default_false")]
    pub calibrated: bool,
    pub calibration_type: Option<CalibrationType>, // Optional: only relevant if calibrated=true
    pub mic_hydro_sensitivity: Option<f64>, // Optional: dB re 1 V/uPa (Wat) or 1 V/Pa (Air)
    pub preamp_gain: Option<f64>,           // Optional: dB
    pub adc_vpeak: Option<f64>,             // Optional: Volts
    pub system_sensitivity: Option<f64>,    // Optional: dB (End-to-end or Recorder sensitivity)

    // DFT/Windowing Settings
    #[serde(default = "default_window_type")]
    pub window_type: WindowType,
    #[serde(default = "default_window_length")]
    pub window_length: f64,
    #[serde(default = "default_window_unit")]
    pub window_unit: WindowUnit,
    #[serde(default = "default_overlap")]
    pub overlap_percentage: f64,

    // Frequency Settings
    pub low_cutoff: f64,                     // Hz
    pub high_cutoff: f64,                    // Hz

    // Optional Settings
    pub welch_factor: Option<usize>,         // Optional: Integer factor for Welch averaging
    pub timestamp_format: Option<String>,    // Optional: Format string for timestamp parsing
}

// Default value functions for serde
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_window_type() -> WindowType { WindowType::Hann }
fn default_window_length() -> f64 { 1.0 }
fn default_window_unit() -> WindowUnit { WindowUnit::Seconds }
fn default_overlap() -> f64 { 50.0 }


// Function to load configuration from a TOML file
pub fn load_config(path: &Path) -> Result<AnalysisConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: AnalysisConfig = toml::from_str(&content)?;

    // Basic validation (more can be added)
    if config.calibrated {
        if config.calibration_type.is_none() {
            return Err("Calibration type must be specified when calibrated=true".into());
        }
        // Add more checks based on calibration_type and required fields
        match config.calibration_type.as_ref().unwrap() {
            CalibrationType::Ts => {
                if config.mic_hydro_sensitivity.is_none() || config.preamp_gain.is_none() || config.adc_vpeak.is_none() {
                     return Err("mic_hydro_sensitivity, preamp_gain, and adc_vpeak must be set for TS calibration type".into());
                }
            },
            CalibrationType::Ee => {
                 if config.system_sensitivity.is_none() {
                     return Err("system_sensitivity must be set for EE calibration type".into());
                 }
            },
             CalibrationType::Rc => {
                 if config.mic_hydro_sensitivity.is_none() || config.system_sensitivity.is_none() {
                     return Err("mic_hydro_sensitivity and system_sensitivity must be set for RC calibration type".into());
                 }
            }
        }
    }
    if config.overlap_percentage < 0.0 || config.overlap_percentage >= 100.0 {
        return Err("overlap_percentage must be between 0.0 and 99.9".into());
    }
     if config.low_cutoff >= config.high_cutoff {
        return Err("low_cutoff must be less than high_cutoff".into());
    }


    Ok(config)
}
