# Pocket Karaoke 🎤

**Turn an Android phone into a low-overhead karaoke microphone for a Windows laptop — over USB or local Wi-Fi.**

Native **Slint desktop UI + Rust/WASAPI audio receiver**, plus a very small **Android microphone APK**. YouTube plays normally in your browser; Windows mixes it with your microphone through your 3.5 mm speakers. No Electron, WebView, virtual audio driver, subscription, or cloud.

[![Build and test](https://github.com/CyGray/pocket-karaoke/actions/workflows/build.yml/badge.svg)](https://github.com/CyGray/pocket-karaoke/actions/workflows/build.yml)

[**Download Windows GUI, CLI & Android APK**](https://github.com/CyGray/pocket-karaoke/releases/latest)

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

## Windows GUI

Double-click **pocket-karaoke.exe**. The new Slint interface provides:
- USB / Wi-Fi mode, fresh six-digit pairing PIN, and receiver start/stop.
- Live Android microphone connected/disconnected status.
- Microphone gain (0–200%, including mute) and a configurable 20–100 ms jitter buffer.
- Windows default speaker device display and a YouTube karaoke shortcut.
- A native, CPU-only software renderer with no embedded browser or heavyweight UI framework.

The [UI source is editable in Slint](ui/app.slint); its Rust event handlers are in [src/gui.rs](src/gui.rs).

### USB setup — recommended

1. Plug your 3.5 mm speakers into the Windows laptop and select them as default output.
2. Enable USB debugging on your Android phone and connect it by cable. Install [ADB platform-tools](https://developer.android.com/tools/releases/platform-tools).
3. Open **pocket-karaoke.exe**. Choose **USB (ADB)**, then **Start microphone receiver**.
4. In a Windows terminal run:

~~~powershell
adb devices
adb reverse tcp:8765 tcp:8765
~~~

5. Install/open **pocket-karaoke-android.apk**, grant microphone permission, and enter **127.0.0.1**, **8765**, and the six-digit pairing PIN shown in the UI.
6. Tap **Connect microphone**. Start any karaoke instrumental on YouTube on the laptop.

USB uses **ADB reverse TCP**, not the USB Audio device protocol. Repeat the ADB command if you disconnect/reconnect the cable.

### Wi-Fi setup

1. Put your laptop and phone on the **same trusted private Wi-Fi**.
2. Open **pocket-karaoke.exe**, select **Wi-Fi**, and press **Start microphone receiver**.
3. Allow the app through Windows Firewall on **Private networks only**.
4. In the Android app, enter the laptop's IPv4 address (the GUI attempts to show it; otherwise use **ipconfig**), port **8765**, and the six-digit PIN.
5. Tap **Connect microphone** and play karaoke in your laptop browser.

While the receiver is running, the connection mode and buffer are locked until you stop it. Gain remains live.

## Optional CLI

The original terminal interface is retained separately as **pocket-karaoke-cli.exe**:

~~~powershell
.\pocket-karaoke-cli.exe --lan
.\pocket-karaoke-cli.exe --buffer-ms 60 --gain 0.8
.\pocket-karaoke-cli.exe --help
~~~

While it runs: commands **gain 0.7**, **mute**, **unmute**, **status**, **quit**. Only run **one receiver** at a time, because GUI and CLI both use TCP port 8765.

## Build from source

Rust on Windows:

~~~powershell
cargo test
cargo build --release --bin pocket-karaoke --bin pocket-karaoke-cli
~~~

Android uses Java 17, Android SDK 35, Gradle 8.9:

~~~sh
gradle -p android assembleDebug lintDebug
~~~

GitHub Actions builds/tests both Windows binaries and the Android APK; tagged releases publish all three files. The downloadable Android APK is debug-signed for personal testing.

## Performance design

- Slint **Winit + software renderer**, with default features disabled. No Skia/FemtoVG GPU renderer or WebView.
- The mic is streamed as PCM16 mono (~96 KB/s at 48 kHz), with no FFmpeg/transcoding.
- Bounded audio queue; 20–100 ms configurable prebuffer; non-blocking audio callback.
- Network accepts run in a small background thread. The UI checks mic status only every 200 ms.
- YouTube music plays independently in the browser and is mixed by the existing Windows audio system.

**Runtime RAM has not been benchmarked on your laptop**. The design is meant to minimize overhead, but binary file size is not a reliable measurement of memory usage.

## Safety and limitations

- **Wi-Fi audio and the pairing PIN are sent without encryption.** Use trusted private networks only; never expose port 8765 to the public internet. USB is preferable on unknown networks.
- Keep the phone away from speakers and begin at low volume to avoid loud acoustic feedback.
- Latency includes Android capture, transport, buffering, and Windows output; USB tends to be steadier but does not guarantee imperceptible delay.
- Android stops recording when you leave the foreground; keep its app open.
- Supports one streaming phone at a time. No recording/uploading to a server, no accounts.

## License

App source: [MIT](LICENSE). The [Slint UI toolkit](https://slint.dev) is used under its [royalty-free desktop license](https://slint.dev/agreements/slint-royalty-free-license.pdf), with the Slint attribution above and the AboutSlint component displayed in the desktop UI. Dependencies have their own licenses.
