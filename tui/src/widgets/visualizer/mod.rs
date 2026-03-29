pub mod audio_capture;
pub mod bar_spectrum;
pub mod circular;
pub mod dot_matrix;
pub mod fire;
pub mod heartbeat;
pub mod kaleidoscope;
pub mod particle_field;
pub mod rainfall;
pub mod scope;
pub mod spectrogram;
pub mod stereo;
pub mod vu_meter;

use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;

/// Trait for pluggable visualizer rendering styles.
/// Implement this trait to add a new visualizer style.
pub trait VisualizerStyle: Send {
    /// Human-readable name shown in the widget title
    fn name(&self) -> &str;

    /// Render the visualization given frequency band data.
    /// `bands` contains normalized amplitude values (0.0 - 1.0).
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        tick: u64,
    );
}
