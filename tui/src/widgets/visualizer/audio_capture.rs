use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::{Arc, Mutex};

const FFT_SIZE: usize = 1024;
const BAND_COUNT: usize = 24;

pub struct AudioCapture {
    pub bands: Arc<Mutex<Vec<f32>>>,
    _stream: Option<cpal::Stream>,
}

impl AudioCapture {
    pub fn new() -> Self {
        let bands = Arc::new(Mutex::new(vec![0.0f32; BAND_COUNT]));
        let stream = Self::start_capture(bands.clone());
        Self {
            bands,
            _stream: stream,
        }
    }

    /// Returns a no-op capture (all bands zero) when audio isn't available
    pub fn silent() -> Self {
        Self {
            bands: Arc::new(Mutex::new(vec![0.0f32; BAND_COUNT])),
            _stream: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self._stream.is_some()
    }

    fn start_capture(bands: Arc<Mutex<Vec<f32>>>) -> Option<cpal::Stream> {
        let host = cpal::default_host();

        // WASAPI loopback: captures system audio output
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;
        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(FFT_SIZE * 2)));
        let buffer_clone = buffer.clone();
        let bands_clone = bands.clone();

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut buf = buffer_clone.lock().unwrap();
                // Mix down to mono and accumulate
                for chunk in data.chunks(channels) {
                    let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    buf.push(mono);
                }
                // Process when we have enough samples
                if buf.len() >= FFT_SIZE {
                    let samples: Vec<f32> = buf.drain(..FFT_SIZE).collect();
                    let result = compute_bands(&samples, sample_rate);
                    if let Ok(mut b) = bands_clone.lock() {
                        // Smooth: 70% new, 30% old for decay effect
                        for (i, val) in result.iter().enumerate() {
                            if i < b.len() {
                                b[i] = val * 0.7 + b[i] * 0.3;
                            }
                        }
                    }
                }
            },
            |err| {
                eprintln!("Audio capture error: {}", err);
            },
            None,
        ).ok()?;

        stream.play().ok()?;
        Some(stream)
    }
}

fn compute_bands(samples: &[f32], sample_rate: f32) -> Vec<f32> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    // Apply Hann window and convert to complex
    let mut input: Vec<Complex<f32>> = samples.iter().enumerate().map(|(i, &s)| {
        let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos());
        Complex::new(s * window, 0.0)
    }).collect();

    fft.process(&mut input);

    // Only use first half (Nyquist)
    let half = FFT_SIZE / 2;
    let magnitudes: Vec<f32> = input[..half].iter()
        .map(|c| (c.re * c.re + c.im * c.im).sqrt() / half as f32)
        .collect();

    // Map to logarithmic frequency bands
    let min_freq = 60.0f32;
    let max_freq = 16000.0f32.min(sample_rate / 2.0);

    let mut bands = vec![0.0f32; BAND_COUNT];
    for (band_idx, band) in bands.iter_mut().enumerate() {
        let f_low = min_freq * (max_freq / min_freq).powf(band_idx as f32 / BAND_COUNT as f32);
        let f_high = min_freq * (max_freq / min_freq).powf((band_idx + 1) as f32 / BAND_COUNT as f32);

        let bin_low = (f_low * FFT_SIZE as f32 / sample_rate).round() as usize;
        let bin_high = (f_high * FFT_SIZE as f32 / sample_rate).round() as usize;
        let bin_low = bin_low.max(1).min(half - 1);
        let bin_high = bin_high.max(bin_low + 1).min(half);

        let sum: f32 = magnitudes[bin_low..bin_high].iter().sum();
        let count = (bin_high - bin_low).max(1) as f32;
        *band = (sum / count * 8.0).min(1.0);
    }

    bands
}
