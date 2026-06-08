//! Sample data operations — extraction, conversion, editing.

use xmrs::prelude::*;

/// Extracted sample data ready for display and editing.
#[derive(Debug, Clone)]
pub struct SampleData {
    /// Sample name.
    pub name: String,
    /// Number of sample frames (mono samples = sample count; stereo = pairs).
    pub length: usize,
    /// Sample rate hint (from the sample's relative note, or 44100 default).
    pub sample_rate: u32,
    /// Bit depth (8, 16, 32).
    pub bits: u8,
    /// Normalized mono samples in [-1.0, 1.0] — ready for waveform display.
    pub mono_data: Vec<f32>,
    /// Whether the source was stereo.
    pub is_stereo: bool,
    /// Stereo right channel data (only if is_stereo).
    pub right_data: Vec<f32>,
    /// Loop type.
    pub loop_type: LoopType,
    /// Loop start in sample frames.
    pub loop_start: u32,
    /// Loop length in sample frames.
    pub loop_length: u32,
    /// Sustain loop type.
    pub sustain_loop_type: LoopType,
    /// Sustain loop start.
    pub sustain_loop_start: u32,
    /// Sustain loop length.
    pub sustain_loop_length: u32,
    /// Relative pitch (C-4 = 0, in semitones).
    pub relative_pitch: i8,
    /// Finetune.
    pub finetune: i8,
    /// Sample volume (0-64).
    pub volume: u8,
}

impl SampleData {
    /// Extract normalized mono data from an xmrs Sample for waveform display.
    pub fn from_sample(sample: &Sample) -> Self {
        let (mono_data, right_data, is_stereo) = match &sample.data {
            Some(SampleDataType::Mono8(v)) => {
                let mono: Vec<f32> = v.iter().map(|&s| s as f32 / 128.0).collect();
                (mono, Vec::new(), false)
            }
            Some(SampleDataType::Mono16(v)) => {
                let mono: Vec<f32> = v.iter().map(|&s| s as f32 / 32768.0).collect();
                (mono, Vec::new(), false)
            }
            Some(SampleDataType::Stereo8(v)) => {
                let left: Vec<f32> = v.iter().step_by(2).map(|&s| s as f32 / 128.0).collect();
                let right: Vec<f32> = v.iter().skip(1).step_by(2).map(|&s| s as f32 / 128.0).collect();
                (left, right, true)
            }
            Some(SampleDataType::Stereo16(v)) => {
                let left: Vec<f32> = v.iter().step_by(2).map(|&s| s as f32 / 32768.0).collect();
                let right: Vec<f32> = v.iter().skip(1).step_by(2).map(|&s| s as f32 / 32768.0).collect();
                (left, right, true)
            }
            Some(SampleDataType::StereoFloat(v)) => {
                let left: Vec<f32> = v.iter().step_by(2).copied().collect();
                let right: Vec<f32> = v.iter().skip(1).step_by(2).copied().collect();
                (left, right, true)
            }
            None => (Vec::new(), Vec::new(), false),
        };

        let length = mono_data.len();
        let bits = sample.bits();

        // Determine sample rate from relative note
        // C-5 notes: middle C is C-5 in xmrs, which is ~8363 Hz at middle-C rate
        // The sample rate is 8363 * 2^(relative_pitch / 12)
        let base_rate = 8363.0;
        let sample_rate = (base_rate * 2.0f64.powf(sample.relative_pitch as f64 / 12.0)) as u32;

        SampleData {
            name: sample.name.clone(),
            length,
            sample_rate: sample_rate.max(1),
            bits,
            mono_data,
            is_stereo,
            right_data,
            loop_type: sample.loop_flag,
            loop_start: sample.loop_start,
            loop_length: sample.loop_length,
            sustain_loop_type: sample.sustain_loop_flag,
            sustain_loop_start: sample.sustain_loop_start,
            sustain_loop_length: sample.sustain_loop_length,
            relative_pitch: sample.relative_pitch,
            finetune: 0, // Q15 format, simplified
            volume: sample.volume.to_byte_64(),
        }
    }

    /// Create a new empty SampleData.
    pub fn empty() -> Self {
        SampleData {
            name: String::new(),
            length: 0,
            sample_rate: 44100,
            bits: 16,
            mono_data: Vec::new(),
            is_stereo: false,
            right_data: Vec::new(),
            loop_type: LoopType::No,
            loop_start: 0,
            loop_length: 0,
            sustain_loop_type: LoopType::No,
            sustain_loop_start: 0,
            sustain_loop_length: 0,
            relative_pitch: 0,
            finetune: 0,
            volume: 64,
        }
    }

    /// Compute a min/max overview for large samples (for peak display).
    /// Returns `(min_values, max_values)` each of length `num_buckets`.
    pub fn overview(&self, num_buckets: usize) -> (Vec<f32>, Vec<f32>) {
        if self.mono_data.is_empty() || num_buckets == 0 {
            return (vec![0.0; num_buckets], vec![0.0; num_buckets]);
        }

        let bucket_size = (self.mono_data.len() as f64 / num_buckets as f64).max(1.0) as usize;
        let mut mins = vec![0.0f32; num_buckets];
        let mut maxs = vec![0.0f32; num_buckets];

        for (i, chunk) in self.mono_data.chunks(bucket_size).enumerate() {
            if i >= num_buckets {
                break;
            }
            let min = chunk.iter().fold(f32::MAX, |a, &b| a.min(b));
            let max = chunk.iter().fold(f32::MIN, |a, &b| a.max(b));
            mins[i] = min;
            maxs[i] = max;
        }

        (mins, maxs)
    }
}

/// Sample editing operations.
impl SampleData {
    /// Normalize sample to full amplitude.
    pub fn normalize(&mut self) {
        if self.mono_data.is_empty() {
            return;
        }
        let max_amp = self
            .mono_data
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, f32::max);
        if max_amp > 0.0 {
            let scale = 1.0 / max_amp;
            for s in &mut self.mono_data {
                *s *= scale;
            }
            for s in &mut self.right_data {
                *s *= scale;
            }
        }
    }

    /// Amplify by a factor.
    pub fn amplify(&mut self, factor: f32) {
        for s in &mut self.mono_data {
            *s = (*s * factor).clamp(-1.0, 1.0);
        }
        for s in &mut self.right_data {
            *s = (*s * factor).clamp(-1.0, 1.0);
        }
    }

    /// Reverse the sample.
    pub fn reverse(&mut self) {
        self.mono_data.reverse();
        self.right_data.reverse();
        // Swap loop points
        let old_start = self.loop_start;
        let old_end = self.loop_start + self.loop_length;
        let len = self.length as u32;
        if old_end > 0 && old_end <= len {
            self.loop_start = len - old_end;
            self.loop_length = old_end - old_start;
        }
    }

    /// Apply fade-in over the first `duration` samples.
    pub fn fade_in(&mut self, duration: usize) {
        for i in 0..duration.min(self.mono_data.len()) {
            let gain = i as f32 / duration as f32;
            self.mono_data[i] *= gain;
        }
    }

    /// Apply fade-out over the last `duration` samples.
    pub fn fade_out(&mut self, duration: usize) {
        let len = self.mono_data.len();
        for i in 0..duration.min(len) {
            let gain = 1.0 - (i as f32 / duration as f32);
            self.mono_data[len - 1 - i] *= gain;
        }
    }

    /// Trim to region [start, end).
    pub fn trim(&mut self, start: usize, end: usize) {
        let end = end.min(self.mono_data.len());
        let start = start.min(end);
        self.mono_data = self.mono_data[start..end].to_vec();
        if self.is_stereo {
            self.right_data = self.right_data[start..end].to_vec();
        }
        self.length = self.mono_data.len();
        // Adjust loop points
        if self.loop_start >= start as u32 {
            self.loop_start -= start as u32;
        } else {
            self.loop_start = 0;
        }
        if self.sustain_loop_start >= start as u32 {
            self.sustain_loop_start -= start as u32;
        } else {
            self.sustain_loop_start = 0;
        }
    }
}
