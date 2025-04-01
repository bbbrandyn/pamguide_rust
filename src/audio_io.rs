use hound::WavReader;
use std::path::Path;

/// Reads a mono WAV audio file and returns its normalized samples (in [-1.0, 1.0]) and the sample rate.
/// Supports 16-bit, 24-bit, 32-bit PCM, and 32-bit float formats.
pub fn read_wav_file(path: &Path) -> Result<(Vec<f32>, u32), Box<dyn std::error::Error>> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();

    if spec.channels != 1 {
        return Err(format!(
            "Unsupported channel count: {}. Only mono files are currently supported.",
            spec.channels
        ).into());
    }

    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Int => match spec.bits_per_sample {
            16 => {
                reader.samples::<i16>()
                    .map(|s| s.map(|v| (v as f32 / i16::MAX as f32).clamp(-1.0, 1.0)))
                    .collect()
            }
            24 => {
                let max_val = (1 << 23) - 1; // 8_388_607
                reader.samples::<i32>()
                    .map(|s| s.map(|v| (v as f32 / max_val as f32).clamp(-1.0, 1.0)))
                    .collect()
            }
            32 => {
                reader.samples::<i32>()
                    .map(|s| s.map(|v| (v as f32 / i32::MAX as f32).clamp(-1.0, 1.0)))
                    .collect()
            }
            _ => return Err(format!("Unsupported integer bit depth: {}", spec.bits_per_sample).into()),
        },
        hound::SampleFormat::Float => match spec.bits_per_sample {
            32 => {
                reader.samples::<f32>()
                    .map(|s| s.map(|v| v.clamp(-1.0, 1.0))) // Clamp just to be safe
                    .collect()
            }
            _ => return Err(format!("Unsupported float bit depth: {}", spec.bits_per_sample).into()),
        },
    };

    let audio_data = samples?;
    Ok((audio_data, spec.sample_rate))
}
