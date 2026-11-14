package com.cygray.pocketkaraoke;

import android.Manifest;
import android.app.Activity;
import android.content.pm.PackageManager;
import android.graphics.Typeface;
import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.MediaRecorder;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.View;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import java.io.BufferedInputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

/**
 * Android's built-in AudioRecord and plain TCP: no WebView, ads, SDK, or third-party libraries.
 * 16-bit mono PCM is captured and sent to the paired Windows receiver in ~20 ms chunks.
 */
public final class MainActivity extends Activity {
    private static final int MIC_PERMISSION_REQUEST = 1001;
    private EditText hostField;
    private EditText portField;
    private EditText pinField;
    private TextView statusText;
    private Button connectButton;
    private Button disconnectButton;
    private volatile boolean streaming;
    private volatile Socket activeSocket;
    private Thread captureThread;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        int space = dp(18);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setPadding(space, dp(28), space, space);

        TextView title = new TextView(this);
        title.setText("Pocket Karaoke 🎤");
        title.setTextSize(27);
        title.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        layout.addView(title);

        TextView help = new TextView(this);
        help.setText("Turn your Android phone into a mic for your Windows PC.\n" +
            "Wi-Fi: enter laptop's local IP and launch receiver with --lan.\n" +
            "USB: run adb reverse tcp:8765 tcp:8765, then use 127.0.0.1.");
        help.setTextSize(14);
        help.setPadding(0, dp(12), 0, dp(14));
        layout.addView(help);

        hostField = field(layout, "Laptop IP or USB loopback", "127.0.0.1", false);
        portField = field(layout, "Port", "8765", true);
        pinField = field(layout, "6-digit pairing PIN (shown on laptop)", "", true);
        pinField.setInputType(InputType.TYPE_CLASS_NUMBER);
        hostField.setText(getPreferences(0).getString("last_host", "127.0.0.1"));

        connectButton = new Button(this);
        connectButton.setText("Connect microphone");
        layout.addView(connectButton);

        disconnectButton = new Button(this);
        disconnectButton.setText("Disconnect");
        disconnectButton.setEnabled(false);
        layout.addView(disconnectButton);

        statusText = new TextView(this);
        statusText.setText("Disconnected");
        statusText.setTextSize(15);
        statusText.setPadding(0, dp(18), 0, 0);
        layout.addView(statusText);

        TextView note = new TextView(this);
        note.setText("Keep this app open while singing. Use your laptop speakers for both YouTube and your voice. " +
            "For the least feedback, place the phone behind the speakers and start at a low volume.");
        note.setTextSize(13);
        note.setPadding(0, dp(24), 0, 0);
        layout.addView(note);

        ScrollView scroller = new ScrollView(this);
        scroller.addView(layout);
        setContentView(scroller);

        connectButton.setOnClickListener(v -> askPermissionAndConnect());
        disconnectButton.setOnClickListener(v -> stopStreaming());
    }

    private EditText field(LinearLayout layout, String label, String value, boolean digits) {
        TextView caption = new TextView(this);
        caption.setText(label);
        caption.setTextSize(14);
        layout.addView(caption);
        EditText edit = new EditText(this);
        edit.setSingleLine(true);
        edit.setText(value);
        edit.setTextSize(17);
        if (digits) {
            edit.setInputType(InputType.TYPE_CLASS_NUMBER);
        }
        layout.addView(edit);
        return edit;
    }

    private int dp(int value) {
        return (int) (value * getResources().getDisplayMetrics().density + 0.5f);
    }

    private void askPermissionAndConnect() {
        if (Build.VERSION.SDK_INT >= 23 &&
            checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, MIC_PERMISSION_REQUEST);
            return;
        }
        connect();
    }

    @Override
    public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request == MIC_PERMISSION_REQUEST) {
            if (results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED) {
                connect();
            } else {
                statusText.setText("Microphone permission is required.");
            }
        }
    }

    private void connect() {
        if (streaming) return;
        String host = hostField.getText().toString().trim();
        String pin = pinField.getText().toString().trim();
        int port;
        try {
            port = Integer.parseInt(portField.getText().toString().trim());
            if (port <= 0 || port > 65535 || host.isEmpty() || !pin.matches("[0-9]{6}")) {
                throw new IllegalArgumentException("Invalid connection settings");
            }
        } catch (RuntimeException e) {
            statusText.setText("Enter a host, valid port, and exactly 6 digits from your laptop.");
            return;
        }
        getPreferences(0).edit().putString("last_host", host).apply();

        streaming = true;
        connectButton.setEnabled(false);
        disconnectButton.setEnabled(true);
        statusText.setText("Connecting to " + host + ":" + port + " ...");

        captureThread = new Thread(() -> capture(host, port, pin), "karaoke-microphone");
        captureThread.start();
    }

    private void capture(String host, int port, String pin) {
        AudioRecord recorder = null;
        Socket socket = new Socket();
        String finished = "Disconnected";
        try {
            // 48 kHz is preferred. Fall back to 44.1 kHz for older devices.
            int rate = 48000;
            int minBytes = AudioRecord.getMinBufferSize(
                rate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
            if (minBytes <= 0) {
                rate = 44100;
                minBytes = AudioRecord.getMinBufferSize(
                    rate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
            }
            if (minBytes <= 0) throw new IOException("Device does not support mono PCM16 recording.");

            int frameBytes = (rate / 50) * 2; // approximately 20 ms per packet
            int recordBufferBytes = Math.max(minBytes * 2, frameBytes * 3);
            recorder = new AudioRecord(
                MediaRecorder.AudioSource.MIC,
                rate,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                recordBufferBytes);
            if (recorder.getState() != AudioRecord.STATE_INITIALIZED) {
                throw new IOException("Could not initialize Android microphone.");
            }

            socket.connect(new InetSocketAddress(host, port), 4000);
            socket.setTcpNoDelay(true);
            socket.setSoTimeout(4000);
            socket.setSendBufferSize(4096);
            activeSocket = socket;
            OutputStream out = socket.getOutputStream();
            String handshake = String.format(Locale.ROOT, "PKARAOKE/1 %s %d\n", pin, rate);
            out.write(handshake.getBytes(StandardCharsets.US_ASCII));
            out.flush();
            String response = readResponse(socket);
            if (!"OK".equals(response)) {
                if ("ERR AUTH".equals(response)) {
                    throw new IOException("Wrong pairing PIN. Recheck the code shown on laptop.");
                }
                if ("ERR BUSY".equals(response)) {
                    throw new IOException("A phone is already connected.");
                }
                throw new IOException("Receiver rejected connection: " + response);
            }

            // Recording only starts AFTER the server authenticates the pairing PIN.
            socket.setSoTimeout(0);
            recorder.startRecording();
            AudioRecord liveRecorder = recorder;
            runOnUiThread(() -> {
                if (streaming) {
                    getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                    statusText.setText("🎤 Connected — microphone live (" +
                        liveRecorder.getSampleRate() + " Hz)");
                }
            });
            byte[] frame = new byte[frameBytes];
            while (streaming) {
                int count = recorder.read(frame, 0, frame.length, AudioRecord.READ_BLOCKING);
                if (count < 0) throw new IOException("Microphone capture failed: " + count);
                if (count > 0) out.write(frame, 0, count);
            }
        } catch (Exception e) {
            finished = "Connection stopped: " + e.getMessage();
        } finally {
            streaming = false;
            activeSocket = null;
            try { socket.close(); } catch (IOException ignored) { }
            if (recorder != null) {
                try {
                    if (recorder.getRecordingState() == AudioRecord.RECORDSTATE_RECORDING) {
                        recorder.stop();
                    }
                } catch (IllegalStateException ignored) { }
                recorder.release();
            }
            String message = finished;
            runOnUiThread(() -> {
                getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                statusText.setText(message);
                connectButton.setEnabled(true);
                disconnectButton.setEnabled(false);
            });
        }
    }

    private String readResponse(Socket socket) throws IOException {
        BufferedInputStream in = new BufferedInputStream(socket.getInputStream(), 32);
        StringBuilder reply = new StringBuilder();
        for (int n = 0; n < 32; n++) {
            int byteValue = in.read();
            if (byteValue == -1 || byteValue == '\n') break;
            reply.append((char) byteValue);
        }
        return reply.toString().trim();
    }

    private void stopStreaming() {
        streaming = false;
        Socket socket = activeSocket;
        if (socket != null) {
            try { socket.close(); } catch (IOException ignored) { }
        }
        statusText.setText("Stopping microphone ...");
    }

    @Override
    protected void onStop() {
        super.onStop();
        stopStreaming(); // privacy: stop recording when leaving the foreground
    }
}
