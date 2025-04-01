use crate::config::WindowType;
use rustfft::{FftPlanner, num_complex::Complex};
use ndarray::{Array1, ArrayView1};
use std::f32::consts::PI;

/// Applies a window function in-place to a segment and returns the scaling factor alpha.


/// Calculates the noise power bandwidth (B) for a given window function.
/// Assumes the window values have already been scaled by alpha.
pub fn noise_power_bandwidth(window_view: ArrayView1<f32>, n_samples: usize) -> f64 {
    // B = (1/N) * sum(w[n]^2) where w[n] is the scaled window
    let sum_sq: f32 = window_view.iter().map(|&x| x.powi(2)).sum();
    (1.0 / n_samples as f64) * sum_sq as f64
}

/// Calculates the FFT of a real-valued segment.
pub fn calculate_fft(segment: &[f32]) -> Vec<Complex<f32>> {
    let n = segment.len();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);

    // Create complex buffer and copy real data into it
    let mut buffer: Vec<Complex<f32>> = segment.iter().map(|&x| Complex::new(x, 0.0)).collect();

    // Perform FFT in-place
    fft.process(&mut buffer);

    buffer
}

/// Generates the window values for a given type and length, scaled by alpha.
pub fn generate_scaled_window(win_type: &WindowType, n_samples: usize) -> (Array1<f32>, f64) {
     let n_f32 = n_samples as f32;
     let (window_values, alpha): (Vec<f32>, f64) = match win_type {
        WindowType::Rectangular => {
            (vec![1.0; n_samples], 1.0)
        }
        WindowType::Hann => {
            let alpha = 0.5;
            let window = (0..n_samples)
                .map(|i| alpha as f32 - 0.5 * (2.0 * PI * i as f32 / n_f32).cos())
                .collect();
            (window, alpha)
        }
        WindowType::Hamming => {
            let alpha = 0.54;
            let window = (0..n_samples)
                .map(|i| alpha as f32 - 0.46 * (2.0 * PI * i as f32 / n_f32).cos())
                .collect();
            (window, alpha)
        }
        WindowType::Blackman => {
            let alpha = 0.42;
            let window = (0..n_samples)
                .map(|i| {
                    alpha as f32 - 0.5 * (2.0 * PI * i as f32 / n_f32).cos()
                        + 0.08 * (4.0 * PI * i as f32 / n_f32).cos()
                })
                .collect();
            (window, alpha)
        }
    };

    // Scale by alpha
    let scaled_window: Vec<f32> = window_values.iter().map(|&x| x / alpha as f32).collect();

    (Array1::from(scaled_window), alpha)
}
