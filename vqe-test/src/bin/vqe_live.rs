// Realtime VQE: reads arecord -t raw S32_LE 2ch on stdin, per-frame
// processing, writes raw mono + processed mono wavs, prints per-second
// levels. This loop is the template for wiring VQE into a voice service.
// usage: vqe_live raw_out.wav vqe_out.wav [config.json]
use std::io::Read;
use std::process::exit;

fn main() {
	let a: Vec<String> = std::env::args().collect();
	if a.len() < 3 {
		eprintln!("usage: {} raw_out.wav vqe_out.wav [config.json]", a[0]);
		exit(1);
	}
	let conf = a
		.get(3)
		.cloned()
		.unwrap_or_else(|| "/opt/microduck-test/vqe-test/config_mic.json".into());

	let mut vqe = match vqe::Vqe::new(&conf) {
		Some(v) => v,
		None => {
			eprintln!("preprocess_init failed");
			exit(1);
		}
	};

	const FRAME_BYTES: usize = vqe::FRAME_PTS * 8; // S32 2ch
	let mut buf = vec![0u8; FRAME_BYTES];
	let mut in2 = vec![0i16; vqe::IN_SHORTS];
	let mut out = vec![0i16; vqe::IN_SHORTS * 2];
	let mut raw: Vec<i16> = Vec::new();
	let mut vqeout: Vec<i16> = Vec::new();
	let mut isq = 0f64;
	let mut osq = 0f64;
	let (mut icnt, mut ocnt, mut frames, mut sec) = (0usize, 0usize, 0usize, 0usize);

	let mut stdin = std::io::stdin().lock();
	'outer: loop {
		let mut got = 0;
		while got < FRAME_BYTES {
			match stdin.read(&mut buf[got..]) {
				Ok(0) | Err(_) => break 'outer,
				Ok(n) => got += n,
			}
		}
		for i in 0..vqe::FRAME_PTS {
			let l = i32::from_le_bytes([
				buf[i * 8],
				buf[i * 8 + 1],
				buf[i * 8 + 2],
				buf[i * 8 + 3],
			]);
			let m = (l >> 16) as i16; // S32 -> high 16 bits
			in2[i * 2] = m;
			in2[i * 2 + 1] = m;
			raw.push(m);
			isq += (m as f64) * (m as f64);
			icnt += 1;
		}
		let n = vqe.process(&in2, &mut out);
		for &s in &out[..n] {
			osq += (s as f64) * (s as f64);
			vqeout.push(s);
			ocnt += 1;
		}
		frames += 1;
		if frames % 62 == 0 {
			// ~1s
			sec += 1;
			let ir = if icnt > 0 { (isq / icnt as f64).sqrt() } else { 0.0 };
			let or = if ocnt > 0 { (osq / ocnt as f64).sqrt() } else { 0.0 };
			eprintln!("[{sec:2}s] in_rms={ir:6.0}  out_rms={or:6.0}");
			isq = 0.0;
			osq = 0.0;
			icnt = 0;
			ocnt = 0;
		}
	}

	if !raw.is_empty() {
		vqe::write_wav_mono(&a[1], &raw).expect("write raw wav");
	}
	if !vqeout.is_empty() {
		vqe::write_wav_mono(&a[2], &vqeout).expect("write vqe wav");
	}
	eprintln!("captured {} samples, processed {}, done", raw.len(), vqeout.len());
}
