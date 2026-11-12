`cargo run -p posara-sfx -- pcm input.wav output.pcm --rate 24000`

WAV/MP3 → mono signed 16-bit PCM; windowed-sinc resampling, DC removal and 1 ms edge fades. Relative recording levels are preserved. PSM1 header: magic (4 bytes), little-endian u32 rate, u32 frame count, then little-endian i16 samples.

VM: `snd_sample(&bytes, 0, volume)` uses the embedded rate; a positive rate overrides playback speed. Native firmware uses `sfx::pcm::Clip` / `Player` with borrowed flash data, without copying samples into RAM. The existing raw 1-bit format remains supported.
