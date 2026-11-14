//! Native Windows receiver. Windows WASAPI's shared-mode mixer combines
//! Chrome/YouTube audio with the microphone audio from this process.

use crate::audio::AudioQueue;
use crate::protocol::{parse_handshake, HandshakeError, HANDSHAKE_MAX, PORT};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use std::error::Error;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

type AppResult<T> = Result<T, Box<dyn Error>>;

struct Options {
    lan: bool,
    port: u16,
    buffer_ms: u32,
    gain: f32,
    list_devices: bool,
}

impl Options {
    fn parse() -> AppResult<Self> {
        let mut opts = Self {
            lan: false,
            port: PORT,
            buffer_ms: 40,
            gain: 1.0,
            list_devices: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--lan" => opts.lan = true,
                "--list-devices" => opts.list_devices = true,
                "--port" => {
                    opts.port = args
                        .next()
                        .ok_or("--port needs a value")?
                        .parse()
                        .map_err(|_| "invalid port")?;
                    if opts.port == 0 {
                        return Err("port must be nonzero".into());
                    }
                }
                "--buffer-ms" => {
                    opts.buffer_ms = args
                        .next()
                        .ok_or("--buffer-ms needs a value")?
                        .parse()
                        .map_err(|_| "invalid buffer value")?;
                    if !(20..=100).contains(&opts.buffer_ms) {
                        return Err("--buffer-ms must be between 20 and 100".into());
                    }
                }
                "--gain" => {
                    opts.gain = args
                        .next()
                        .ok_or("--gain needs a value")?
                        .parse()
                        .map_err(|_| "invalid gain")?;
                    if !opts.gain.is_finite() || !(0.0..=3.0).contains(&opts.gain) {
                        return Err("--gain must be between 0 and 3".into());
                    }
                }
                "--help" | "-h" => {
                    help();
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown argument: {arg} (try --help)").into()),
            }
        }
        Ok(opts)
    }
}

fn help() {
    println!(
        "Pocket Karaoke v{}\n\
         A phone-to-speaker microphone receiver for Windows.\n\n\
         Usage:\n\
           pocket-karaoke.exe                  USB / loopback only (safe default)\n\
           pocket-karaoke.exe --lan            Accept phones on local Wi-Fi\n\
           pocket-karaoke.exe --buffer-ms 60   Increase jitter buffer (20-100 ms)\n\
           pocket-karaoke.exe --gain 1.2       Initial microphone gain (0-3)\n\
           pocket-karaoke.exe --port 8765      Change TCP port\n\
           pocket-karaoke.exe --list-devices   List audio output devices\n\n\
         Commands while running: gain 0.8 | mute | unmute | status | quit\n\
         Pair your Android app using the 6-digit PIN printed at startup.\n\
         Your YouTube tab plays separately through the Windows audio mixer.",
        env!("CARGO_PKG_VERSION")
    );
}

pub fn run() -> AppResult<()> {
    let opts = Options::parse()?;
    let host = cpal::default_host();
    if opts.list_devices {
        for device in host.output_devices()? {
            println!("{}", device.name()?);
        }
        return Ok(());
    }

    let device = host.default_output_device().ok_or(
        "No default output device. Plug speakers into 3.5mm and select them in Windows Sound settings.",
    )?;
    let default = device.default_output_config()?;
    let config: StreamConfig = default.config();
    let channels = config.channels as usize;
    let output_rate = config.sample_rate.0;
    let shared = Arc::new(Mutex::new(AudioQueue::new(
        48000,
        output_rate,
        opts.buffer_ms,
    )));
    let gain = Arc::new(AtomicU32::new(opts.gain.to_bits()));
    let connected = Arc::new(AtomicBool::new(false));

    let stream = open_audio_stream(
        &device,
        &default.sample_format(),
        &config,
        channels,
        &shared,
        &gain,
    )?;
    stream.play()?;

    let mut seed = [0u8; 4];
    getrandom::getrandom(&mut seed)
        .map_err(|e| format!("Cannot obtain random pairing PIN: {e}"))?;
    let pin = format!("{:06}", u32::from_le_bytes(seed) % 1_000_000);
    let address = format!(
        "{}:{}",
        if opts.lan { "0.0.0.0" } else { "127.0.0.1" },
        opts.port
    );
    let listener = TcpListener::bind(&address)?;

    println!(
        "Pocket Karaoke v{} [native / no Electron]",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "Output: {} — {} Hz, {} channel(s)",
        device.name()?,
        output_rate,
        channels
    );
    println!("Listening: {address}");
    println!("Phone pairing PIN: {pin}");
    if opts.lan {
        println!(
            "Wi-Fi: enter your laptop's private IPv4 address (from ipconfig), port {}, and PIN.",
            opts.port
        );
        println!("Only permit this program through Windows Firewall on PRIVATE networks.");
    } else {
        println!(
            "USB: enable USB debugging, then run: adb reverse tcp:{} tcp:{}",
            opts.port, opts.port
        );
        println!(
            "Android app: host 127.0.0.1, port {}, PIN shown above.",
            opts.port
        );
        println!("For Wi-Fi, restart this receiver with --lan.");
    }
    println!("Open YouTube karaoke in your browser; Windows mixes music with microphone playback.");
    println!("Commands: gain <0-3> | mute | unmute | status | quit");
    println!(
        "Tip: avoid speaker feedback: keep phone away from speakers and begin at a low volume."
    );

    let command_gain = gain.clone();
    let command_connected = connected.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lines() {
            let Ok(line) = line else {
                return;
            };
            let mut bits = line.split_whitespace();
            match bits.next() {
                Some("gain") => match bits.next().and_then(|n| n.parse::<f32>().ok()) {
                    Some(v) if v.is_finite() && (0.0..=3.0).contains(&v) => {
                        command_gain.store(v.to_bits(), Ordering::Relaxed);
                        println!("Microphone gain: {v:.2}");
                    }
                    _ => println!("Usage: gain <0.0 to 3.0>"),
                },
                Some("mute") => {
                    command_gain.store(0.0f32.to_bits(), Ordering::Relaxed);
                    println!("Microphone muted.");
                }
                Some("unmute") => {
                    command_gain.store(1.0f32.to_bits(), Ordering::Relaxed);
                    println!("Microphone gain: 1.0");
                }
                Some("status") => println!(
                    "Phone: {} | gain: {:.2}",
                    if command_connected.load(Ordering::Relaxed) {
                        "connected"
                    } else {
                        "disconnected"
                    },
                    f32::from_bits(command_gain.load(Ordering::Relaxed))
                ),
                Some("quit" | "exit") => std::process::exit(0),
                Some(_) | None => println!("Commands: gain <0-3> | mute | unmute | status | quit"),
            }
        }
    });

    // Keep the stream alive while serving TCP connections.
    let _stream = stream;
    for accepted in listener.incoming() {
        match accepted {
            Ok(socket) => {
                let shared = shared.clone();
                let connected = connected.clone();
                let pin = pin.clone();
                let buffer_ms = opts.buffer_ms;
                thread::spawn(move || {
                    if let Err(e) = handle_phone(socket, &pin, shared, connected, buffer_ms) {
                        eprintln!("Phone connection: {e}");
                    }
                });
            }
            Err(e) => eprintln!("TCP listener: {e}"),
        }
    }
    Ok(())
}

fn open_audio_stream(
    device: &cpal::Device,
    format: &SampleFormat,
    config: &StreamConfig,
    channels: usize,
    shared: &Arc<Mutex<AudioQueue>>,
    gain: &Arc<AtomicU32>,
) -> AppResult<Stream> {
    let q = shared.clone();
    let g = gain.clone();
    let stream = match format {
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _| render(data, channels, &q, &g, 0.0, |sample| sample),
            audio_error,
            None,
        )?,
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _| {
                render(data, channels, &q, &g, 0, |sample| {
                    (sample * 32767.0) as i16
                })
            },
            audio_error,
            None,
        )?,
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], _| {
                render(data, channels, &q, &g, 32768, |sample| {
                    ((sample + 1.0) * 32767.5) as u16
                })
            },
            audio_error,
            None,
        )?,
        other => return Err(format!("Unsupported output sample format: {other:?}").into()),
    };
    Ok(stream)
}

fn audio_error(error: cpal::StreamError) {
    eprintln!("Audio backend: {error}");
}

fn render<T: Copy>(
    output: &mut [T],
    channels: usize,
    shared: &Arc<Mutex<AudioQueue>>,
    gain: &Arc<AtomicU32>,
    silence: T,
    convert: impl Fn(f32) -> T,
) {
    // A callback must NEVER wait for a network thread. Silence instead.
    let Ok(mut queue) = shared.try_lock() else {
        output.fill(silence);
        return;
    };
    let volume = f32::from_bits(gain.load(Ordering::Relaxed));
    for frame in output.chunks_mut(channels) {
        let mono = (queue.next_sample() * volume).clamp(-0.98, 0.98);
        let sample = convert(mono);
        frame.fill(sample);
    }
}

struct PhoneLease {
    busy: Arc<AtomicBool>,
    shared: Arc<Mutex<AudioQueue>>,
}
impl Drop for PhoneLease {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.shared.lock() {
            queue.clear();
        }
        self.busy.store(false, Ordering::Release);
        println!("Phone disconnected.");
    }
}

fn handle_phone(
    mut socket: TcpStream,
    pin: &str,
    shared: Arc<Mutex<AudioQueue>>,
    connected: Arc<AtomicBool>,
    buffer_ms: u32,
) -> io::Result<()> {
    socket.set_nodelay(true)?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut header = Vec::with_capacity(HANDSHAKE_MAX);
    let mut one = [0u8; 1];
    loop {
        if header.len() >= HANDSHAKE_MAX {
            socket.write_all(b"ERR FORMAT\n")?;
            return Ok(());
        }
        if socket.read(&mut one)? == 0 {
            return Ok(());
        }
        if one[0] == b'\n' {
            break;
        }
        header.push(one[0]);
    }
    let text = match std::str::from_utf8(&header) {
        Ok(t) => t,
        Err(_) => {
            socket.write_all(b"ERR FORMAT\n")?;
            return Ok(());
        }
    };
    let rate = match parse_handshake(text, pin) {
        Ok(rate) => rate,
        Err(HandshakeError::Authentication) => {
            socket.write_all(b"ERR AUTH\n")?;
            thread::sleep(Duration::from_millis(250)); // slow down wrong PIN guesses
            return Ok(());
        }
        Err(HandshakeError::Format) => {
            socket.write_all(b"ERR FORMAT\n")?;
            return Ok(());
        }
    };
    if connected
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        socket.write_all(b"ERR BUSY\n")?;
        return Ok(());
    }
    let _lease = PhoneLease {
        busy: connected,
        shared: shared.clone(),
    };
    if let Ok(mut queue) = shared.lock() {
        queue.set_source_rate(rate, buffer_ms);
    }
    socket.write_all(b"OK\n")?;
    socket.set_read_timeout(Some(Duration::from_secs(3)))?;
    println!("Phone connected: {} Hz, mono PCM16", rate);

    let mut input = [0u8; 4096];
    let mut pcm = Vec::with_capacity(4098);
    let mut tail = None;
    loop {
        let bytes = socket.read(&mut input)?;
        if bytes == 0 {
            break;
        }
        pcm.clear();
        if let Some(byte) = tail.take() {
            pcm.push(byte);
        }
        pcm.extend_from_slice(&input[..bytes]);
        if pcm.len() % 2 == 1 {
            tail = pcm.pop();
        }
        if let Ok(mut queue) = shared.lock() {
            queue.append_pcm16(&pcm);
        }
    }
    Ok(())
}
