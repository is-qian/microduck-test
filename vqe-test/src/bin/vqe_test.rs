// Offline VQE (ANR+AGC) on a 16k mono s16 wav.
// usage: vqe_test in_mono.wav out.wav [config.json]
use std::process::exit;

fn main() {
	let a: Vec<String> = std::env::args().collect();
	if a.len() < 3 {
		eprintln!("usage: {} in_mono.wav out.wav [config.json]", a[0]);
		exit(1);
	}
	let conf = a
		.get(3)
		.cloned()
		.unwrap_or_else(|| "/opt/microduck-test/vqe-test/config_mic.json".into());

	let mono = match vqe::read_wav_mono(&a[1]) {
		Ok(m) if !m.is_empty() => m,
		_ => {
			eprintln!("bad or empty input wav");
			exit(1);
		}
	};
	eprintln!("mono samples={} conf={}", mono.len(), conf);

	let mut vqe = match vqe::Vqe::new(&conf) {
		Some(v) => v,
		None => {
			eprintln!("preprocess_init failed");
			exit(1);
		}
	};

	let in2 = vqe::expand_2ch(&mono);
	let mut out = vec![0i16; vqe::IN_SHORTS * 2];
	let mut obuf = Vec::with_capacity(mono.len());
	let mut frames = 0usize;
	for off in (0..in2.len()).step_by(vqe::IN_SHORTS) {
		if off + vqe::IN_SHORTS > in2.len() {
			break;
		}
		let n = vqe.process(&in2[off..off + vqe::IN_SHORTS], &mut out);
		obuf.extend_from_slice(&out[..n]);
		frames += 1;
	}
	eprintln!("frames={} output shorts={}", frames, obuf.len());

	vqe::write_wav_mono(&a[2], &obuf).expect("write out wav");
	eprintln!("done");
}
