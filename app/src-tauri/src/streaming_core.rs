//! Pure audio segmentation and WAV encoding, testable without the desktop runtime.

const MIN_SEGMENT_SECONDS: usize = 5;
const MAX_SEGMENT_SECONDS: usize = 27;

pub(crate) struct AudioChunk {
    pub(crate) sample_rate: u32,
    pub(crate) samples: Vec<f32>,
}

#[derive(Default)]
pub(crate) struct SegmentPlanner {
    pub(crate) sample_rate: u32,
    pending: Vec<f32>,
}

impl SegmentPlanner {
    pub(crate) fn push(&mut self, chunk: AudioChunk) -> Result<Option<Vec<f32>>, String> {
        if self.sample_rate == 0 {
            self.sample_rate = chunk.sample_rate;
        }
        if chunk.sample_rate == 0 || chunk.sample_rate != self.sample_rate {
            return Err("Cambió la frecuencia del micrófono durante el dictado".into());
        }
        self.pending.extend_from_slice(&chunk.samples);
        let seconds = self.pending.len() / self.sample_rate as usize;
        if seconds >= MAX_SEGMENT_SECONDS && !self.trailing_silence() {
            // A forced cut can split a word: use the complete-audio fallback instead.
            return Err("No se halló una pausa segura; se usará el audio completo".into());
        }
        if seconds >= MIN_SEGMENT_SECONDS && self.trailing_silence() {
            return Ok(Some(std::mem::take(&mut self.pending)));
        }
        Ok(None)
    }

    fn trailing_silence(&self) -> bool {
        let count = (self.sample_rate as usize * 400 / 1_000).max(1);
        if self.pending.len() < count {
            return false;
        }
        rms(&self.pending[self.pending.len() - count..]) < 0.008
    }

    pub(crate) fn finish(&mut self) -> Option<Vec<f32>> {
        if self.pending.is_empty() || rms(&self.pending) < 0.003 {
            self.pending.clear();
            None
        } else {
            Some(std::mem::take(&mut self.pending))
        }
    }
}

pub(crate) fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples
        .iter()
        .map(|sample| (*sample as f64).powi(2))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt() as f32
}

pub(crate) fn pcm_wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let bytes = samples
        .len()
        .checked_mul(2)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or("El fragmento de audio es demasiado grande")?;
    let riff_size = 36_u32.checked_add(bytes).ok_or("El WAV es demasiado grande")?;
    let mut wav = Vec::with_capacity(44 + bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&16_000_u32.to_le_bytes());
    wav.extend_from_slice(&32_000_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&bytes.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16).to_le_bytes());
    }
    Ok(wav)
}

#[cfg(test)]
mod tests {
    use super::{pcm_wav, AudioChunk, SegmentPlanner};

    #[test]
    fn segments_at_a_real_pause_and_preserves_frames() {
        let mut planner = SegmentPlanner::default();
        assert!(planner
            .push(AudioChunk { sample_rate: 16_000, samples: vec![0.1; 16_000 * 5] })
            .unwrap()
            .is_none());
        let first = planner
            .push(AudioChunk { sample_rate: 16_000, samples: vec![0.0; 16_000 / 2] })
            .unwrap()
            .unwrap();
        assert_eq!(first.len(), 88_000);
        assert!(planner
            .push(AudioChunk { sample_rate: 16_000, samples: vec![0.2; 16_000 * 2] })
            .unwrap()
            .is_none());
        assert_eq!(planner.finish().unwrap().len(), 32_000);
    }

    #[test]
    fn never_cuts_uninterrupted_speech() {
        let mut planner = SegmentPlanner::default();
        assert!(planner
            .push(AudioChunk { sample_rate: 16_000, samples: vec![0.1; 16_000 * 27] })
            .is_err());
    }

    #[test]
    fn rejects_rate_changes() {
        let mut planner = SegmentPlanner::default();
        planner.push(AudioChunk { sample_rate: 16_000, samples: vec![0.1; 16_000] }).unwrap();
        assert!(planner.push(AudioChunk { sample_rate: 48_000, samples: vec![0.1; 48_000] }).is_err());
    }

    #[test]
    fn wav_header_is_mono_16khz_pcm() {
        let bytes = pcm_wav(&[0.0, 1.0]).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 4);
        assert_eq!(bytes.len(), 48);
    }
}
