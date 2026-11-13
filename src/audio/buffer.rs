//! Reuse the interleaved conversion buffer instead of allocating and copying every packet.
use std::ops::Deref;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};

#[derive(Default)]
pub(super) struct DecodedSamples {
    samples: Option<SampleBuffer<f32>>,
    #[cfg(test)]
    allocations: usize,
}
impl DecodedSamples {
    pub fn copy(&mut self, decoded: AudioBufferRef<'_>) {
        let required = decoded.capacity() * decoded.spec().channels.count();
        if self
            .samples
            .as_ref()
            .is_none_or(|s| s.capacity() < required)
        {
            self.samples = Some(SampleBuffer::new(
                decoded.capacity() as u64,
                *decoded.spec(),
            ));
            #[cfg(test)]
            {
                self.allocations += 1;
            }
        }
        self.samples.as_mut().unwrap().copy_interleaved_ref(decoded);
    }
    pub fn clear(&mut self) {
        if let Some(samples) = &mut self.samples {
            samples.clear();
        }
    }
}
impl Deref for DecodedSamples {
    type Target = [f32];
    fn deref(&self) -> &[f32] {
        self.samples.as_ref().map_or(&[], SampleBuffer::samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{borrow::Cow, hint::black_box, time::Instant};
    use symphonia::core::audio::{AudioBuffer, Channels, Signal, SignalSpec};

    fn packet(frames: usize, stereo: bool) -> AudioBuffer<f32> {
        let spec = SignalSpec::new(
            48_000,
            if stereo {
                Channels::FRONT_LEFT | Channels::FRONT_RIGHT
            } else {
                Channels::FRONT_LEFT
            },
        );
        let mut b = AudioBuffer::new(frames as u64, spec);
        b.render_reserved(None);
        for channel in 0..spec.channels.count() {
            for (i, s) in b.chan_mut(channel).iter_mut().enumerate() {
                *s = (i as f32 + channel as f32) / 8192.;
            }
        }
        b
    }
    #[test]
    fn changing_packet_sizes_and_channels_preserve_every_sample_and_reuse_storage() {
        let mut reused = DecodedSamples::default();
        for (frames, stereo) in [(1024, true), (4096, true), (512, false), (2048, true)] {
            let b = packet(frames, stereo);
            let mut original = SampleBuffer::<f32>::new(b.capacity() as u64, *b.spec());
            original.copy_interleaved_ref(AudioBufferRef::F32(Cow::Borrowed(&b)));
            reused.copy(AudioBufferRef::F32(Cow::Borrowed(&b)));
            assert_eq!(&*reused, original.samples());
            reused.clear();
            assert!(reused.is_empty());
        }
        assert_eq!(reused.allocations, 2);
    }
    #[test]
    #[ignore = "Explicit before/after conversion microbenchmark; not a timing assertion"]
    fn conversion_benchmark() {
        let b = packet(4096, true);
        let packets = 10_000;
        let mut old_times = Vec::new();
        let mut new_times = Vec::new();
        for round in 0..9 {
            // Alternate order so neither implementation always benefits from warm caches.
            for old in if round % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            } {
                let mut output = Vec::new();
                let mut reused = DecodedSamples::default();
                let start = Instant::now();
                for _ in 0..packets {
                    if old {
                        let mut samples = SampleBuffer::<f32>::new(b.capacity() as u64, *b.spec());
                        samples.copy_interleaved_ref(AudioBufferRef::F32(Cow::Borrowed(&b)));
                        output.clear();
                        output.extend_from_slice(samples.samples());
                        black_box(output.as_slice());
                    } else {
                        reused.copy(AudioBufferRef::F32(Cow::Borrowed(&b)));
                        black_box(&*reused);
                    }
                }
                let elapsed = start.elapsed().as_secs_f64() * 1000.;
                if old {
                    old_times.push(elapsed);
                } else {
                    assert_eq!(reused.allocations, 1);
                    new_times.push(elapsed);
                }
            }
        }
        old_times.sort_by(f64::total_cmp);
        new_times.sort_by(f64::total_cmp);
        let result = serde_json::json!({"scope":"4096-frame stereo f32 conversion/interleaving, 10000 packets per batch, nine alternating-order batches. Original allocation+copy algorithm versus the production reusable buffer; not whole-player CPU or RAM.","old_median_ms":old_times[4],"new_median_ms":new_times[4],"speedup":old_times[4]/new_times[4],"old_conversion_buffer_allocations_per_batch":packets,"new_conversion_buffer_allocations_per_batch":1,"removed_redundant_sample_bytes":4096*2*4});
        println!("{result}");
        if let Ok(path) = std::env::var("UNDERTOW_BENCH_REPORT") {
            crate::config::write_json(std::path::Path::new(&path), &result).unwrap();
        }
    }
}
