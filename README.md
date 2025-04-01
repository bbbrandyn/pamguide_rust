# PAMGuide Rust Implementation

This project is a Rust implementation of parts of the PAMGuide MATLAB toolbox. PAMGuide is designed for the analysis of passive acoustic monitoring data, providing tools for calculating calibrated sound levels.

This Rust version aims to provide similar functionality to the original MATLAB PAMGuide, potentially offering performance improvements as well as replacing the GUI with a config file which enables preservation of settings.

**Currently Implemented Analysis Types:**

*   Broadband Sound Pressure Levels (SPL)
*   Power Spectral Density (PSD)

## Cloning the Repository

To get a local copy of this project, clone the repository using Git:

```bash
git clone https://github.com/bbbrandyn/pamguide_rust.git
cd pamguide_rust
```

## Running the Project

To run the analysis, you need to have Rust and Cargo installed. You can find installation instructions at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

Once Rust is installed, you can run the project from the `pamguide_rust` directory using Cargo:

```bash
cargo run --release
```

The `--release` flag is recommended for better performance.

## Configuration

The analysis settings are controlled via the `config.toml` file located in the project's root directory (`pamguide_rust/config.toml`).

Before running the analysis, modify this file to specify:

*   **Input audio file path:** The `.wav` file to be analyzed.
*   **Output directory path:** Where the results (e.g., CSV files) will be saved.
*   **Analysis parameters:** Such as calibration values, window size, overlap, frequency band limits, etc., specific to the Broadband and PSD calculations.

Refer to the comments within `config.toml` for details on each parameter.

## Disclaimer

**Please note:** The code in this repository was mostly generated using Gemini 2.5 Pro Experimental. While efforts have been made to ensure correctness, it may contain errors or deviate significantly from the original MATLAB implementation, especially as only a subset of features is included. Use with caution and verify results independently.
