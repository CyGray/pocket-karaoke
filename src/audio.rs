//! Small bounded PCM queue and linear resampler.
//!
//! Kept independent of the audio backend so the DSP can be tested on Linux.
//! The playback thread must not allocate or block.

use std::collections::VecDeque;

pub struct AudioQueue {
    samples: VecDeque<f32>,
    source_rate: u32,
    output_rate: u32,
    prebuffer: usize,
    max_buffer: usize,
    current: f32,
    next: f32,
    phase: f64,
    started: bool,
}

impl AudioQueue {
    pub fn new(source_rate: u32, output_rate: u32, buffer_ms: u32) -> Self {
        let prebuffer = ((source_rate as u64 * buffer_ms as u64) / 1000).max(2) as usize;
        let max_buffer = ((source_rate as u64 * 200) / 1000) as usize;
        Self {
            samples: VecDeque::with_capacity(max_buffer + 2048),
            source_rate,
            output_rate,
            prebuffer,
            max_buffer,
            current: 0.0,
            next: 0.0,
            phase: 0.0,
            started: false,
        }
    }

    pub fn set_source_rate(&mut self, rate: u32, buffer_ms: u32) {
        self.source_rate = rate;
        self.prebuffer = ((rate as u64 * buffer_ms as u64) / 1000).max(2) as usize;
        self.max_buffer = ((rate as u64 * 200) / 1000) as usize;
        self.clear();
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.started = false;
        self.phase = 0.0;
        self.current = 0.0;
        self.next = 0.0;
    }

    pub fn append_pcm16(&mut self, pcm: &[u8]) {
        for pair in pcm.chunks_exact(2) {
            let sample = i16::from_le_bytes([pair[0], pair[1]]);
            self.samples.push_back(sample as f32 / 32768.0);
        }
        // Bound memory and latency even when the phone's audio clock runs fast.
        if self.samples.len() > self.max_buffer {
            let keep = self.prebuffer.saturating_mul(2).min(self.samples.len());
            while self.samples.len() > keep {
                self.samples.pop_front();
            }
            self.started = false; // start again with a short, fresh prebuffer
        }
    }

    /// Called once per output *frame*, not once per channel.
    pub fn next_sample(&mut self) -> f32 {
        if !self.started {
            if self.samples.len() < self.prebuffer {
                return 0.0;
            }
            self.current = self.samples.pop_front().unwrap_or(0.0);
            self.next = self.samples.pop_front().unwrap_or(0.0);
            self.phase = 0.0;
            self.started = true;
        }

        let value = self.current + (self.next - self.current) * self.phase as f32;
        self.phase += self.source_rate as f64 / self.output_rate as f64;

        while self.phase >= 1.0 {
            let Some(sample) = self.samples.pop_front() else {
                self.started = false;
                self.phase = 0.0;
                return value;
            };
            self.current = self.next;
            self.next = sample;
            self.phase -= 1.0;
        }

        value
    }

    #[cfg(test)]
    pub fn buffered_samples(&self) -> usize {
        self.samples.len()
    }
}

#[cfg(test)]
mod tests {
    use super::AudioQueue;

    fn pcm(samples: &[i16]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    #[test]
    fn silence_until_prebuffer_is_met() {
        let mut q = AudioQueue::new(1000, 1000, 4);
        q.append_pcm16(&pcm(&[100, 200, 300]));
        assert_eq!(q.next_sample(), 0.0);
        q.append_pcm16(&pcm(&[400, 500]));
        assert!((q.next_sample() - 100.0 / 32768.0).abs() < 0.0001);
    }

    #[test]
    fn linear_interpolation_on_upsampling() {
        let mut q = AudioQueue::new(1000, 2000, 2);
        q.append_pcm16(&pcm(&[0, 16000, 32000, 32000]));
        assert_eq!(q.next_sample(), 0.0);
        assert!((q.next_sample() - 8000.0 / 32768.0).abs() < 0.0001);
        assert!((q.next_sample() - 16000.0 / 32768.0).abs() < 0.0001);
    }

    #[test]
    fn buffer_has_strict_limit_even_with_large_input() {
        let mut q = AudioQueue::new(48000, 48000, 40);
        q.append_pcm16(&pcm(&vec![1000; 25000]));
        assert!(q.buffered_samples() <= 48000 * 2 * 40 / 1000);
    }

    #[test]
    fn resets_when_a_new_phone_connects() {
        let mut q = AudioQueue::new(48000, 48000, 40);
        q.append_pcm16(&pcm(&[1200; 3000]));
        q.set_source_rate(44100, 40);
        assert_eq!(q.buffered_samples(), 0);
        assert_eq!(q.next_sample(), 0.0);
    }

    #[test]
    fn underflow_returns_to_silence() {
        let mut q = AudioQueue::new(1000, 1000, 2);
        q.append_pcm16(&pcm(&[1234, 2345]));
        for _ in 0..20 {
            q.next_sample();
        }
        assert_eq!(q.next_sample(), 0.0);
    }
}
