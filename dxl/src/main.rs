//! dxl - Dynamixel Protocol 2.0 bus tool for the MicroDuck bus
//! (/dev/ttyS3: GD32 power board at ID 200 + XL330 servos). Zero
//! dependencies: stty configures the port, transactions are plain file I/O.

use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::process::{exit, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

const HDR: [u8; 4] = [0xff, 0xff, 0xfd, 0x00];
const PING_TIMEOUT: Duration = Duration::from_millis(150);
const TXN_TIMEOUT: Duration = Duration::from_millis(300);
const O_NONBLOCK: i32 = 0o4000;

fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x8005 } else { crc << 1 };
        }
    }
    crc
}

// Byte stuffing per protocol 2.0: insert 0xFD after each FF FF FD pattern.
// The length field and the CRC cover the stuffed bytes.
fn add_stuffing(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut run = 0;
    for &b in data {
        out.push(b);
        if run < 2 && b == 0xff {
            run += 1;
        } else if run == 2 && b == 0xfd {
            out.push(0xfd);
            run = 0;
        } else if b == 0xff {
            run = 1;
        } else {
            run = 0;
        }
    }
    out
}

fn remove_stuffing(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut run = 0;
    for &b in data {
        if run == 3 {
            run = 0;
            if b == 0xfd {
                continue;
            }
        }
        if run < 2 && b == 0xff {
            run += 1;
        } else if run == 2 && b == 0xfd {
            run = 3;
        } else if b == 0xff {
            run = 1;
        } else {
            run = 0;
        }
        out.push(b);
    }
    out
}

fn instruction(id: u8, instr: u8, params: &[u8]) -> Vec<u8> {
    let params = add_stuffing(params);
    let length = (params.len() + 3) as u16;
    let mut pkt = Vec::with_capacity(10 + params.len());
    pkt.extend_from_slice(&HDR);
    pkt.push(id);
    pkt.extend_from_slice(&length.to_le_bytes());
    pkt.push(instr);
    pkt.extend_from_slice(&params);
    let crc = crc16(&pkt);
    pkt.extend_from_slice(&crc.to_le_bytes());
    pkt
}

// Ok(Some) = complete valid status packet; Ok(None) = need more bytes;
// Err(n) = drop n leading garbage bytes and resync.
fn parse_status(buf: &[u8]) -> Result<Option<(u8, u8, Vec<u8>)>, usize> {
    let Some(pos) = buf.windows(4).position(|w| w == HDR) else {
        return Err(buf.len().saturating_sub(3));
    };
    if pos > 0 {
        return Err(pos);
    }
    if buf.len() < 7 {
        return Ok(None);
    }
    let len = u16::from_le_bytes([buf[5], buf[6]]) as usize;
    // status = instruction + error + params + crc16, so at least 4
    if !(4..=68).contains(&len) {
        return Err(4);
    }
    if buf.len() < 7 + len {
        return Ok(None);
    }
    let pkt = &buf[..7 + len];
    if pkt[7] != 0x55 {
        return Err(4);
    }
    if crc16(&pkt[..5 + len]) != u16::from_le_bytes([pkt[5 + len], pkt[6 + len]]) {
        return Err(4);
    }
    Ok(Some((pkt[4], pkt[8], remove_stuffing(&pkt[9..5 + len]))))
}

struct Port {
    file: File,
}

impl Port {
    fn open(path: &str, baud: u32) -> io::Result<Port> {
        // O_NONBLOCK: termios is per-port; another opener (robotd retry) would reset stty's settings
        let status = Command::new("stty")
            .arg("-F")
            .arg(path)
            .arg(baud.to_string())
            .args(["raw", "-echo", "-icanon", "min", "0", "time", "1"])
            .status()?;
        if !status.success() {
            return Err(io::Error::other("stty failed"));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(O_NONBLOCK)
            .open(path)?;
        Ok(Port { file })
    }

    // Drop pending input so a late reply is not attributed to the next txn.
    fn drain(&mut self) {
        let mut buf = [0u8; 256];
        loop {
            match self.file.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }

    // Returns (id, error, params) of the status packet, None on timeout.
    fn txn(&mut self, pkt: &[u8], timeout: Duration) -> Option<(u8, u8, Vec<u8>)> {
        self.drain();
        let mut out = pkt;
        while !out.is_empty() {
            match self.file.write(out) {
                Ok(n) => out = &out[n..],
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => sleep(Duration::from_millis(1)),
                Err(_) => return None,
            }
        }
        let deadline = Instant::now() + timeout;
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            // USB-TTL adapters echo our TX; the board's auto-direction bus does not
            if buf.len() >= pkt.len() && buf[..pkt.len()] == *pkt {
                buf.drain(..pkt.len());
            }
            match parse_status(&buf) {
                Ok(Some(r)) => return Some(r),
                Ok(None) => {}
                Err(0) => {}
                Err(n) => {
                    buf.drain(..n);
                    continue;
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            match self.file.read(&mut chunk) {
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => sleep(Duration::from_millis(1)),
                Err(_) => return None,
            }
        }
    }
}

// Some((model, fw, err)); err bit7 is the hardware-alert flag.
fn ping(port: &mut Port, id: u8) -> Option<(u16, u8, u8)> {
    let (rid, err, data) = port.txn(&instruction(id, 0x01, &[]), PING_TIMEOUT)?;
    if rid != id || data.len() < 3 {
        return None;
    }
    Some((u16::from_le_bytes([data[0], data[1]]), data[2], err))
}

fn read(port: &mut Port, id: u8, addr: u16, len: u16) -> Option<(u8, Vec<u8>)> {
    let mut params = Vec::with_capacity(4);
    params.extend_from_slice(&addr.to_le_bytes());
    params.extend_from_slice(&len.to_le_bytes());
    let (rid, err, data) = port.txn(&instruction(id, 0x02, &params), TXN_TIMEOUT)?;
    if rid != id {
        return None;
    }
    Some((err, data))
}

fn write(port: &mut Port, id: u8, addr: u16, data: &[u8]) -> Option<u8> {
    let mut params = Vec::with_capacity(2 + data.len());
    params.extend_from_slice(&addr.to_le_bytes());
    params.extend_from_slice(data);
    let (rid, err, _) = port.txn(&instruction(id, 0x03, &params), TXN_TIMEOUT)?;
    if rid != id {
        return None;
    }
    Some(err)
}

fn reboot(port: &mut Port, id: u8) {
    port.txn(&instruction(id, 0x08, &[]), TXN_TIMEOUT);
    sleep(Duration::from_millis(500));
}

fn f16_to_f32(h: u16) -> f32 {
    let sign = (h >> 15) & 1;
    let exp = (h >> 10) & 0x1f;
    let frac = h & 0x3ff;
    let v = match exp {
        0 => frac as f32 * 2f32.powi(-24),
        0x1f => {
            if frac == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => (1.0 + frac as f32 / 1024.0) * 2f32.powi(exp as i32 - 15),
    };
    if sign == 1 {
        -v
    } else {
        v
    }
}

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let s: String = s.split_whitespace().collect();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

// Baud Rate(8) register encoding for XL330
fn baud_code(baud: u32) -> Option<u8> {
    Some(match baud {
        9600 => 0,
        57600 => 1,
        115200 => 2,
        1_000_000 => 3,
        2_000_000 => 4,
        3_000_000 => 5,
        4_000_000 => 6,
        4_500_000 => 7,
        _ => return None,
    })
}

fn parse_ini(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name.trim().to_string();
            out.entry(section.clone()).or_default();
        } else if let Some((k, v)) = line.split_once('=') {
            if !section.is_empty() {
                out.entry(section.clone())
                    .or_default()
                    .insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    out
}

// Clear Shutdown bits for latched hw errors, reboot to apply (EEPROM).
fn clear_errors(port: &mut Port, id: u8) {
    let hw = match read(port, id, 70, 1) {
        Some((_, d)) if !d.is_empty() => d[0],
        _ => {
            println!("warning: hw_err read failed, skip error clearing");
            return;
        }
    };
    if hw == 0 {
        println!("no hw errors");
        return;
    }
    let shutdown = match read(port, id, 63, 1) {
        Some((_, d)) if !d.is_empty() => d[0],
        _ => {
            println!("warning: shutdown read failed, skip error clearing");
            return;
        }
    };
    let keep = shutdown & !hw;
    if keep != shutdown {
        write(port, id, 64, &[0]);
        sleep(Duration::from_millis(100));
        write(port, id, 63, &[keep]);
        reboot(port, id);
        if ping(port, id).is_none() {
            println!("warning: no answer after reboot");
            return;
        }
    }
    match read(port, id, 70, 1) {
        Some((_, d)) if !d.is_empty() => {
            println!("hw_err 0x{hw:02x} -> 0x{:02x}, shutdown {shutdown} -> {keep}", d[0])
        }
        _ => println!("warning: hw_err re-read failed"),
    }
}

fn configure_group(path: &str, cur_baud: u32, cur_id: u8, tgt_baud: u32, tgt_id: u8) -> bool {
    let mut port = open_or_exit(path, cur_baud);
    if ping(&mut port, cur_id).is_none() {
        drop(port);
        let mut port = open_or_exit(path, tgt_baud);
        if let Some((model, _, _)) = ping(&mut port, tgt_id) {
            println!("already configured: ID {tgt_id} @{tgt_baud}, model {model}");
            clear_errors(&mut port, tgt_id);
            return true;
        }
        println!("skip: no motor @{cur_baud} ID {cur_id} (nor @{tgt_baud} ID {tgt_id})");
        return false;
    }
    println!("found ID {cur_id} @{cur_baud}");
    write(&mut port, cur_id, 64, &[0]); // ID/baud live in EEPROM
    sleep(Duration::from_millis(100));
    let mut id = cur_id;
    if tgt_id != cur_id {
        match write(&mut port, cur_id, 7, &[tgt_id]) {
            Some(0) | Some(128) => {}
            _ => {
                println!("error: write ID failed");
                return false;
            }
        }
        println!("ID {cur_id} -> {tgt_id}");
        id = tgt_id;
        sleep(Duration::from_millis(100));
    }
    if tgt_baud != cur_baud {
        match write(&mut port, id, 8, &[baud_code(tgt_baud).unwrap()]) {
            Some(0) | Some(128) => {}
            _ => {
                println!("error: write baud failed");
                return false;
            }
        }
        println!("baud {cur_baud} -> {tgt_baud}");
        sleep(Duration::from_millis(100));
    }
    drop(port);
    let mut port = open_or_exit(path, tgt_baud);
    match ping(&mut port, tgt_id) {
        Some((model, _, _)) => println!("verify OK: ID {tgt_id} @{tgt_baud}, model {model}"),
        None => {
            println!("error: verify failed, no answer @{tgt_baud} ID {tgt_id}");
            return false;
        }
    }
    clear_errors(&mut port, tgt_id);
    true
}

fn open_or_exit(path: &str, baud: u32) -> Port {
    match Port::open(path, baud) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("dxl: {path}: {e}");
            exit(1);
        }
    }
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize, what: &str) -> T {
    match args.get(i).and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => {
            eprintln!("dxl: bad or missing {what}");
            usage();
        }
    }
}

fn usage() -> ! {
    eprintln!("dxl - Dynamixel Protocol 2.0 bus tool (GD32 power board + XL330 servos)");
    eprintln!("usage: dxl [-p PORT] [-b BAUD] <command> [args...]");
    eprintln!("  -p, --port PORT   serial port (env DXL_PORT, default /dev/ttyS3)");
    eprintln!("  -b, --baud BAUD   bus baud rate (env DXL_BAUD, default 1000000)");
    eprintln!("  dxl scan [baud...]            ping IDs 0..252 (default bauds: 1000000 57600)");
    eprintln!("  dxl ping <id>                 ping 10 times, report model/fw/err");
    eprintln!("  dxl read <id> <addr> <len>    print err + data hex");
    eprintln!("  dxl write <id> <addr> <hex>   write bytes, print err");
    eprintln!("  dxl reboot <id>               reboot, then ping");
    eprintln!("  dxl fixvolt <id>              clear Shutdown bit0 (voltage) + bit1 (overheating), reboot, recheck");
    eprintln!("  dxl move <id> [sec] [deg]     swing 2048 +/-deg every 0.8s (default 60s, 15 deg), return to zero and torque off at end");
            eprintln!("  dxl sensors [id]              IMU board block read @124, decoded (layout auto-detected, default id 200)");
    eprintln!("  dxl setup <new_id>            factory servo (ID 1 @57600): set ID + 1 Mbps; refuses if");
    eprintln!("                                more than one device answers at 57600");
    eprintln!("  dxl configure [conf]          batch-configure per [groupN] sections (default:");
    eprintln!("                                motor.conf next to the dxl binary)");
    exit(2);
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    // Global options come before the subcommand; CLI beats env beats default.
    let mut port_opt = None;
    let mut baud_opt = None;
    while let Some(flag) = args.first().map(String::as_str) {
        let take = |args: &mut Vec<String>| -> String {
            args.remove(0);
            if args.is_empty() {
                usage();
            }
            args.remove(0)
        };
        match flag {
            "-p" | "--port" => port_opt = Some(take(&mut args)),
            "-b" | "--baud" => baud_opt = Some(take(&mut args)),
            _ => break,
        }
    }
    if args.is_empty() {
        usage();
    }
    let explicit_port = port_opt.is_some() || env::var("DXL_PORT").is_ok();
    let path = port_opt
        .or_else(|| env::var("DXL_PORT").ok())
        .unwrap_or_else(|| "/dev/ttyS3".into());
    let baud: u32 = baud_opt
        .and_then(|v| v.parse().ok())
        .or_else(|| env::var("DXL_BAUD").ok().and_then(|v| v.parse().ok()))
        .unwrap_or(1_000_000);

    match args[0].as_str() {
        "scan" => {
            let bauds: Vec<u32> = if args.len() > 1 {
                args[1..].iter().map(|s| s.parse().unwrap_or_else(|_| usage())).collect()
            } else {
                vec![1_000_000, 57600]
            };
            for baud in bauds {
                let mut port = open_or_exit(&path, baud);
                let mut found = Vec::new();
                for id in 0..=252u8 {
                    if let Some((model, _, _)) = ping(&mut port, id) {
                        found.push((id, model));
                    }
                }
                println!("@{baud}: {found:?}");
            }
        }
        "ping" => {
            let id: u8 = arg(&args, 1, "id");
            let mut port = open_or_exit(&path, baud);
            let (mut ok, mut last) = (0, None);
            for _ in 0..10 {
                let r = ping(&mut port, id);
                ok += r.is_some() as u32;
                last = r;
            }
            match last {
                Some((model, fw, err)) => println!("{ok}/10, last=(model {model}, fw {fw}, err {err})"),
                None => println!("{ok}/10, last=no answer"),
            }
            if ok == 0 {
                exit(1);
            }
        }
        "read" => {
            let id: u8 = arg(&args, 1, "id");
            let addr: u16 = arg(&args, 2, "addr");
            let len: u16 = arg(&args, 3, "len");
            let mut port = open_or_exit(&path, baud);
            match read(&mut port, id, addr, len) {
                Some((err, data)) => println!("err={err} data={}", to_hex(&data)),
                None => {
                    println!("no answer");
                    exit(1);
                }
            }
        }
        "write" => {
            let id: u8 = arg(&args, 1, "id");
            let addr: u16 = arg(&args, 2, "addr");
            let data = args.get(3).and_then(|s| parse_hex(s)).unwrap_or_else(|| {
                eprintln!("dxl: bad or missing hex data");
                usage();
            });
            let mut port = open_or_exit(&path, baud);
            match write(&mut port, id, addr, &data) {
                Some(err) => println!("err={err}"),
                None => {
                    println!("no answer");
                    exit(1);
                }
            }
        }
        "reboot" => {
            let id: u8 = arg(&args, 1, "id");
            let mut port = open_or_exit(&path, baud);
            reboot(&mut port, id);
            match ping(&mut port, id) {
                Some((model, fw, err)) => println!("ping: model {model}, fw {fw}, err {err}"),
                None => {
                    println!("ping: no answer");
                    exit(1);
                }
            }
        }
        "fixvolt" => {
            let id: u8 = arg(&args, 1, "id");
            let mut port = open_or_exit(&path, baud);
            write(&mut port, id, 64, &[0]);
            sleep(Duration::from_millis(100));
            let shutdown = match read(&mut port, id, 63, 1) {
                Some((_, d)) if !d.is_empty() => d[0],
                _ => {
                    println!("no answer reading Shutdown(63)");
                    exit(1);
                }
            };
            if let Some((_, d)) = read(&mut port, id, 146, 1) {
                if !d.is_empty() {
                    println!("present temperature: {} C", d[0]);
                }
            }
            println!("shutdown: {shutdown:#08b} (bit0 voltage, bit1 overheating) — clearing both");
            write(&mut port, id, 63, &[shutdown & !0b11]);
            reboot(&mut port, id);
            match read(&mut port, id, 70, 1) {
                Some((_, d)) if !d.is_empty() => println!("hw_err after fix+reboot: {}", d[0]),
                _ => {
                    println!("hw_err after fix+reboot: ?");
                    exit(1);
                }
            }
        }
        "move" => {
            let id: u8 = arg(&args, 1, "id");
            let sec: u64 = if args.len() > 2 { arg(&args, 2, "sec") } else { 60 };
            let mut port = open_or_exit(&path, baud);
            // Zero-centered swing, 4096 counts/rev
            const CENTER: u32 = 2048;
            let deg: f32 = if args.len() > 3 { arg(&args, 3, "deg") } else { 15.0 };
            let amp = (deg * 4096.0 / 360.0).round() as u32;
            if amp >= CENTER {
                eprintln!("dxl: swing +/-{deg} deg out of range");
                exit(1);
            }
            let (mut n, mut bad) = (0u32, 0u32);
            let go = |port: &mut Port, goal: u32, bad: &mut u32| {
                // 0 = ok, 128 = alert bit only; anything else is a real error
                match write(port, id, 116, &goal.to_le_bytes()) {
                    Some(0) | Some(128) => {}
                    _ => *bad += 1,
                }
            };
            write(&mut port, id, 64, &[1]);
            go(&mut port, CENTER, &mut bad);
            sleep(Duration::from_millis(800));
            let start = Instant::now();
            let mut goal = CENTER + amp;
            while start.elapsed().as_secs() < sec {
                go(&mut port, goal, &mut bad);
                goal = if goal == CENTER + amp { CENTER - amp } else { CENTER + amp };
                n += 1;
                sleep(Duration::from_millis(800));
            }
            go(&mut port, CENTER, &mut bad);
            sleep(Duration::from_millis(800));
            write(&mut port, id, 64, &[0]);
            println!("{n} moves, {bad} errors");
            if bad > 0 {
                exit(1);
            }
        }
        "sensors" => {
            let id: u8 = if args.len() > 1 { arg(&args, 1, "id") } else { 200 };
            let mut port = open_or_exit(&path, baud);
            // Layout depends on firmware: zephyr-gd32 (model 19780 = 0x4D44)
            // extends the block with temperature@146 + battery mV@147;
            // imu_to_dxl (model 1200, virtual XL330 table) ends at
            // voltage@144 (22 bytes) and errors on reads past its table.
            let zephyr = matches!(ping(&mut port, id), Some((19780, _, _)));
            let resp = if zephyr {
                // 147/148 (battery mV) missing on early zephyr-gd32 builds
                match read(&mut port, id, 124, 25) {
                    Some((e, d)) if d.len() >= 25 => Some((e, d)),
                    _ => read(&mut port, id, 124, 23),
                }
            } else {
                read(&mut port, id, 124, 22)
            };
            let (err, data) = match resp {
                Some((e, d)) if d.len() >= 22 => (e, d),
                Some((_, d)) => {
                    eprintln!("short reply: {}", to_hex(&d));
                    exit(1);
                }
                None => {
                    eprintln!("no answer");
                    exit(1);
                }
            };
            let i16_at = |o: usize| i16::from_le_bytes([data[o], data[o + 1]]);
            let f16_at = |o: usize| f16_to_f32(u16::from_le_bytes([data[o], data[o + 1]]));
            let (gx, gy, gz) = (i16_at(0) as f32 * 0.0175, i16_at(2) as f32 * 0.0175, i16_at(4) as f32 * 0.0175);
            let (qx, qy, qz) = (f16_at(6), f16_at(8), f16_at(10));
            let (ax, ay, az) = (i16_at(12) as f32 * 0.000122, i16_at(14) as f32 * 0.000122, i16_at(16) as f32 * 0.000122);
            let status = data[19];
            let voltage = u16::from_le_bytes([data[20], data[21]]) as f32 * 0.1;
            println!("err={err}");
            println!("gyro:  {gx:+.2} {gy:+.2} {gz:+.2} dps");
            println!("quat:  {qx:+.4} {qy:+.4} {qz:+.4}");
            println!("accel: {ax:+.3} {ay:+.3} {az:+.3} g");
            println!("sample_counter: {}", data[18]);
            println!(
                "status: 0x{status:02x} (imu_online={}, quat_valid={}, fifo_overflow={})",
                status & 1 != 0,
                status & 2 != 0,
                status & 4 != 0
            );
            println!("voltage: {voltage:.1} V");
            if zephyr && data.len() >= 23 {
                println!("temperature: {} C", data[22]);
                if data.len() >= 25 {
                    let battery_mv = u16::from_le_bytes([data[23], data[24]]);
                    println!("battery: {battery_mv} mV");
                }
            }
            if status & 1 == 0 {
                exit(3); // IMU offline
            }
        }
        "configure" => {
            let conf = match args.get(1) {
                Some(p) => p.clone(),
                None => env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("motor.conf")))
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "motor.conf".into()),
            };
            let text = match std::fs::read_to_string(&conf) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("dxl: {conf}: {e}");
                    exit(1);
                }
            };
            let ini = parse_ini(&text);
            let base_port = match (explicit_port, ini.get("motor").and_then(|m| m.get("port"))) {
                (false, Some(p)) => p.clone(),
                _ => path.clone(),
            };
            let mut groups = Vec::new();
            for i in 1..=16u32 {
                let Some(g) = ini.get(&format!("group{i}")) else {
                    continue;
                };
                let get = |k: &str| -> u32 {
                    match g.get(k).and_then(|v| v.parse().ok()) {
                        Some(v) => v,
                        None => {
                            eprintln!("dxl: group{i}: bad or missing {k}");
                            exit(2);
                        }
                    }
                };
                let (cur_baud, cur_id) = (get("current_baud"), get("current_id"));
                let (tgt_baud, tgt_id) = (get("target_baud"), get("target_id"));
                if baud_code(cur_baud).is_none() || baud_code(tgt_baud).is_none() {
                    eprintln!("dxl: group{i}: unsupported baud");
                    exit(2);
                }
                if cur_id > 252 || tgt_id > 252 {
                    eprintln!("dxl: group{i}: id must be 0..=252");
                    exit(2);
                }
                groups.push((format!("group{i}"), cur_baud, cur_id as u8, tgt_baud, tgt_id as u8));
            }
            if groups.is_empty() {
                eprintln!("dxl: no [groupN] sections in {conf}");
                exit(2);
            }
            let mut failed = 0;
            for (name, cur_baud, cur_id, tgt_baud, tgt_id) in groups {
                println!("== {name}: {cur_baud}/ID{cur_id} -> {tgt_baud}/ID{tgt_id} ==");
                if !configure_group(&base_port, cur_baud, cur_id, tgt_baud, tgt_id) {
                    failed += 1;
                }
            }
            if failed > 0 {
                eprintln!("{failed} group(s) not configured");
                exit(1);
            }
        }
        "setup" => {
            let new_id: u8 = arg(&args, 1, "new_id");
            if !(1..=252).contains(&new_id) {
                eprintln!("dxl: new_id must be 1..=252");
                exit(2);
            }
            // A factory servo is ID 1 @ 57600. Writing EEPROM on the wrong
            // servo is unrecoverable here, so first prove the bus at 57600
            // holds exactly one device and it is ID 1.
            let mut port = open_or_exit(&path, 57600);
            let mut found = Vec::new();
            for id in 1..=252u8 {
                if ping(&mut port, id).is_some() {
                    found.push(id);
                }
            }
            if found != [1] {
                println!("expected a single factory servo at ID 1 @57600, found {found:?}");
                exit(1);
            }
            write(&mut port, 1, 64, &[0]); // torque off: ID/baud live in EEPROM
            sleep(Duration::from_millis(100));
            if new_id != 1 {
                match write(&mut port, 1, 7, &[new_id]) {
                    Some(0) | Some(128) => {}
                    _ => {
                        println!("write ID failed");
                        exit(1);
                    }
                }
            }
            sleep(Duration::from_millis(100));
            // Baud Rate(8) = 3 -> 1 Mbps; takes effect immediately
            match write(&mut port, new_id, 8, &[3]) {
                Some(0) | Some(128) => {}
                _ => {
                    println!("write baud failed");
                    exit(1);
                }
            }
            sleep(Duration::from_millis(100));
            drop(port);
            let mut port = open_or_exit(&path, 1_000_000);
            match ping(&mut port, new_id) {
                Some((model, fw, err)) => {
                    println!("ok: ID {new_id} @1000000, model {model}, fw {fw}, err {err}")
                }
                None => {
                    println!("verify failed: no answer from ID {new_id} @1000000");
                    exit(1);
                }
            }
        }
        _ => usage(),
    }
}
