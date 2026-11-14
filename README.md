# Pocket Karaoke 🎤

**Turn an Android phone into a wireless or USB karaoke microphone for your Windows laptop.**

![CI](https://github.com/CyGray/pocket-karaoke/actions/workflows/build.yml/badge.svg)

A tiny **native Rust** desktop executable plays the phone's microphone through your Windows default audio device. YouTube plays in Chrome as usual; **Windows' shared audio mixer combines the song and your voice** through your 3.5 mm speakers.

No Electron, webview, Voicemeeter, DroidCam, FFmpeg, browser server, accounts, advertising, or cloud services.

## Downloads

[**Download the Windows EXE and Android APK**](https://github.com/CyGray/pocket-karaoke/releases/latest)

The Windows program is portable. The Android app uses native Java/Android APIs, with no extra dependencies. Windows and Android builds also appear as artifacts under [GitHub Actions](https://github.com/CyGray/pocket-karaoke/actions/workflows/build.yml).

## 1. Get the audio working over Wi-Fi

1. Plug speakers into your laptop's **3.5 mm jack**. In Windows Sound settings, select those speakers as the **default output** (the same device Chrome uses).
2. Connect phone and laptop to the **same trusted, private Wi-Fi** network.
3. On your laptop, open a terminal in the folder containing the downloaded EXE and run:
   ```powershell
.\pocket-karaoke.exe --lan
```
4. If Windows Firewall asks, allow **Private networks only**. Do **not** open your router's ports to the internet.
5. In a Windows terminal run `ipconfig` and find your laptop's **IPv4 Address**, typically `192.168.1.x` or `192.168.0.x`.
6. Install/open the Android APK, grant microphone permission, and enter:
   - **Host**: the laptop's IPv4 address
   - **Port**: `8765`
   - **PIN**: the fresh six-digit number printed by the EXE
7. Tap **Connect microphone**. Open YouTube karaoke in your laptop browser, adjust Chrome's volume normally, and sing.

The phone app keeps the screen awake while you're connected and stops the microphone when you leave the app.

## 2. USB connection (recommended for steadier audio)

USB avoids Wi-Fi congestion. It uses Android Debug Bridge (ADB) reverse port forwarding; it is **not** a USB audio device.

1. Enable Android **Developer options → USB debugging**, and connect your phone by USB.
2. Install [Android platform-tools (ADB)](https://developer.android.com/tools/releases/platform-tools) on Windows.
3. Start the Windows receiver **without** `--lan`:
   ```powershell
.\pocket-karaoke.exe
```
4. In another terminal run:
   ```powershell
adb devices
adb reverse tcp:8765 tcp:8765
```
   Accept the USB debugging authorization dialog on the phone.
5. In the phone app enter **Host `127.0.0.1`**, **Port `8765`**, and the receiver's PIN, then tap Connect.

If you reconnect the cable, rerun `adb reverse`.

## Controls and audio

Run ```powershell
.\pocket-karaoke.exe --help
.\pocket-karaoke.exe --list-devices
.\pocket-karaoke.exe --gain 0.8 --buffer-ms 60 --lan
```

While the receiver is running, type `gain 0.7`, `mute`, `unmute`, `status`, or `quit` in its console.

- **YouTube music volume:** adjust in the browser or Windows volume mixer. The Rust process intentionally does not capture system audio.
- **Microphone volume:** `gain` from 0.0 to 3.0 (gain over 1.0 can clip).
- **Network jitter:** `--buffer-ms 40` is the default, adjustable 20–100. Smaller buffers reduce delay but make Wi-Fi more glitch-prone.
- **Speakers:** Windows default WASAPI shared output. The app outputs the same mono mic signal to every channel.

### Limitations

- **Latency cannot be zero.** Android AudioRecord, network/USB transport, jitter buffering and Windows output all add delay. Start with USB and a 20–40 ms jitter buffer if feedback or echo is bothersome.
- **No microphone feedback cancellation.** Keep the phone away from the speakers and use modest volumes. Feedback at high volume can be unpleasant and potentially harmful to hearing.
- **Foreground only:** leaving the Android app intentionally stops recording. Keep the app onscreen during singing.
- **Trusted networks only:** Wi-Fi audio and the six-digit PIN travel as **unencrypted TCP**. Never expose port 8765 to the internet or use this on untrusted/shared Wi-Fi; use USB there.
- **One microphone at a time** in this MVP. PCM16 mono at 48kHz uses roughly **96 KB/s** of network payload. A fallback uses 44.1kHz.
- **Windows-only receiver** at this time. Linux runs pure Rust unit tests but no output device is accessed in CI.
- The project has not been measured on your particular laptop or phone. Do not assume any specific RAM or end-to-end latency figure until you benchmark it locally.

## Architecture

```text
Android app (Java AudioRecord, mono PCM16)
           │ 20ms uncompressed PCM frames
           ▼
    TCP via private Wi-Fi OR ADB reverse USB
           │ 6-digit pairing PIN, one active phone
           ▼
Windows pocket-karaoke.exe (native Rust)
  → bounded 200ms queue / configurable prebuffer
  → lightweight linear resampling to output device Hz
  → CPAL / Windows WASAPI shared device
           │
           ├───────── Windows audio mixer ◄── Chrome / YouTube
           ▼
       3.5 mm speakers
```

Only standard-library networking threads and a small bounded PCM queue are used on the Rust receiver. The real-time audio callback does not block on networking or allocate. The Android client is Java using the platform `AudioRecord` API and a socket; it doesn't require a WebView, Kotlin runtime, or any external runtime dependency.

The application does **not** download, modify, or redistribute YouTube audio; it simply relies on the user's normal browser playback.

## Developing

### Rust receiver

Windows with current stable Rust:

```powershell
cargo test
cargo build --release
.\target\release\pocket-karaoke.exe --lan
```

On Linux, `cargo test` checks the bounded audio queue/resampler and authentication protocol without requiring an ALSA device.

### Android app

Open the `android/` folder in Android Studio, or with JDK 17, Android SDK 35, and Gradle 8.9:

```sh
gradle -p android assembleDebug
```

APK: `android/app/build/outputs/apk/debug/app-debug.apk`.

GitHub Actions compiles/tests both platforms and attaches Windows EXE + Android APK to versioned GitHub releases.

## License

MIT — see [LICENSE](LICENSE).
