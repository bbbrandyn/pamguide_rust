mod config;
mod audio_io;
mod dsp;
mod analysis;
mod utils;
mod broadband_test;

use clap::Parser;
use std::path::PathBuf;
use std::process;

/// PAMGuide Rust - Acoustic analysis tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to input audio file or directory
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
    
    /// Run broadband test to debug dB discrepancy
    #[arg(long)]
    broadband_test: bool,
    
    /// Path to WAV file for broadband test (required if --broadband-test is used)
    #[arg(long)]
    test_wav: Option<PathBuf>,
}

fn main() {
    // Parse command-line arguments
    let args = Args::parse();

    // Check if we should run the broadband test
    if args.broadband_test {
        let test_wav = match &args.test_wav {
            Some(path) => path.to_string_lossy().to_string(),
            None => {
                eprintln!("Error: --test-wav is required when using --broadband-test");
                process::exit(1);
            }
        };
        
        let config_path = args.config.to_string_lossy().to_string();
        
        match broadband_test::run_broadband_test(&test_wav, &config_path) {
            Ok(_) => {
                println!("Broadband test completed successfully.");
                return;
            },
            Err(e) => {
                eprintln!("Broadband test failed: {}", e);
                process::exit(1);
            }
        }
    }

    // Load configuration
    let config_path = args.config;
    let config = match config::load_config(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration from '{}': {}", config_path.display(), e);
            process::exit(1);
        }
    };
    println!("Configuration loaded successfully.");

    // Determine input path (command-line arg overrides config)
    let input_path = match args.input {
        Some(path) => path,
        None => PathBuf::from(&config.input_path),
    };
    println!("Effective input path: {}", input_path.display());


    // Check if input path exists
    if !input_path.exists() {
        eprintln!("Error: Input path '{}' does not exist", input_path.display());
        process::exit(1);
    }

    // Check if input is file or directory and call appropriate handler
    let result = if input_path.is_file() {
        analysis::process_single_file(&input_path, &config)
    } else if input_path.is_dir() {
        analysis::process_directory(&input_path, &config)
    } else {
        eprintln!("Error: Input path '{}' is neither a file nor a directory", input_path.display());
        process::exit(1);
    };

    // Handle potential errors from analysis functions
    if let Err(e) = result {
        eprintln!("Analysis failed: {}", e);
        process::exit(1);
    }

    println!("Analysis finished successfully.");
}
