// Thin FFI to librkaudio_vqe. Only the init_by_conf path works: the
// manual-param init() silently outputs zeros in this lib build.
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

pub const RATE: usize = 16000;
pub const FRAME_PTS: usize = 256; // 16ms @16k, per VQE guide
pub const IN_SHORTS: usize = FRAME_PTS * 2; // 2ch interleaved per call

#[allow(non_snake_case)]
mod ffi {
	use super::*;
	extern "C" {
		pub fn rkaudio_preprocess_init_by_conf(
			rate: c_int, bits: c_int, src_chan: c_int, ref_chan: c_int,
			json: *const c_char,
		) -> *mut c_void;
		pub fn rkaudio_preprocess_short(
			st: *mut c_void, input: *const i16, output: *mut i16,
			in_size: c_int, wakeup: *mut c_int,
		) -> c_int; // in_size and return are shorts, despite the guide saying bytes
		pub fn rkaudio_preprocess_destory(st: *mut c_void);
	}
}

pub struct Vqe {
	st: *mut c_void,
}

impl Vqe {
	pub fn new(conf: &str) -> Option<Vqe> {
		let c = CString::new(conf).ok()?;
		let st = unsafe {
			ffi::rkaudio_preprocess_init_by_conf(
				RATE as c_int, 16, 2, 0, c.as_ptr(),
			)
		};
		(!st.is_null()).then_some(Vqe { st })
	}

	/// input: IN_SHORTS interleaved samples. Returns output SHORTS count
	/// (the C return is bytes: 512 per 256-sample frame, verified on board).
	pub fn process(&mut self, input: &[i16], output: &mut [i16]) -> usize {
		let mut wk: c_int = 0;
		let n = unsafe {
			ffi::rkaudio_preprocess_short(
				self.st, input.as_ptr(), output.as_mut_ptr(),
				input.len() as c_int, &mut wk,
		 )
		};
		(n.max(0) as usize) / 2
	}
}

impl Drop for Vqe {
	fn drop(&mut self) {
		unsafe { ffi::rkaudio_preprocess_destory(self.st) };
	}
}

/// mono -> 2ch interleaved (the PDM capture shape the lib expects)
pub fn expand_2ch(mono: &[i16]) -> Vec<i16> {
	let mut v = Vec::with_capacity(mono.len() * 2);
	for &s in mono {
		v.push(s);
		v.push(s);
	}
	v
}

/// minimal s16 mono wav reader (skips the 44-byte header)
pub fn read_wav_mono(path: &str) -> std::io::Result<Vec<i16>> {
	let b = std::fs::read(path)?;
	let n = (b.len().saturating_sub(44)) / 2;
	Ok(b[44..44 + n * 2]
		.chunks_exact(2)
		.map(|c| i16::from_le_bytes([c[0], c[1]]))
		.collect())
}

pub fn write_wav_mono(path: &str, d: &[i16]) -> std::io::Result<()> {
	use std::io::Write;
	let mut f = std::fs::File::create(path)?;
	f.write_all(b"RIFF")?;
	f.write_all(&((36 + d.len() * 2) as u32).to_le_bytes())?;
	f.write_all(b"WAVEfmt ")?;
	f.write_all(&16u32.to_le_bytes())?;
	f.write_all(&1u16.to_le_bytes())?; // PCM
	f.write_all(&1u16.to_le_bytes())?; // mono
	f.write_all(&(RATE as u32).to_le_bytes())?;
	f.write_all(&((RATE * 2) as u32).to_le_bytes())?;
	f.write_all(&2u16.to_le_bytes())?; // block align
	f.write_all(&16u16.to_le_bytes())?;
	f.write_all(b"data")?;
	f.write_all(&((d.len() * 2) as u32).to_le_bytes())?;
	for s in d {
		f.write_all(&s.to_le_bytes())?;
	}
	Ok(())
}
