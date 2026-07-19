<div align="center">
  <h1>🎙️ SpeakType</h1>
  <p><strong>A lightning-fast, privacy-first, cross-application dictation tool for macOS & Linux.</strong></p>
  
  [![Rust](https://img.shields.io/badge/Rust-Daemon-orange.svg)](https://rust-lang.org)
  [![FastAPI](https://img.shields.io/badge/FastAPI-Service-009688.svg)](https://fastapi.tiangolo.com)
  [![Whisper.cpp](https://img.shields.io/badge/Whisper-Local-blue.svg)](https://github.com/ggerganov/whisper.cpp)
  [![Gemini](https://img.shields.io/badge/Gemini-Flash_Lite-purple.svg)](https://ai.google.dev/)
  
  <br />
  <a href="#how-it-works">How it Works</a> •
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a>
</div>

---

<div align="center">
  <img src="https://via.placeholder.com/800x450.png?text=[Insert+Demo+Video/GIF+Here]" alt="SpeakType Demo Video">
  <p><em>Hold a hotkey anywhere on your desktop, speak, and release. Watch the transcript magically type itself out.</em></p>
</div>

## 💡 What is SpeakType?

Imagine having a super-smart assistant that types exactly what you mean, anywhere on your computer. With SpeakType, you just hold a shortcut key (`ALT+SPACE`), speak your mind naturally, and let go. 

The app instantly types out your words directly into whichever app you are using—whether you're sending a casual text in WhatsApp, drafting a professional email in Outlook, or brainstorming in Apple Notes. It automatically understands the context of the app you're in and formats your spoken words perfectly to match the situation, all while keeping your raw voice recordings 100% private and on your device.

## ✨ Features

- **100% Local Voice Recognition**: Uses `whisper.cpp` to process your voice directly on your device. No audio ever leaves your computer.
- **Context-Aware AI Formatting**: Automatically detects which app you are currently using (e.g., Slack, VS Code, Notes) and uses Google's ultra-fast `gemini-flash-lite` to apply the perfect tone, punctuation, and formatting.
- **Global Hotkey Support**: Works everywhere. You don't need to install app-specific extensions. Just hold `ALT+SPACE` (or your custom hotkey) and start talking.
- **Real-Time Streaming Injection**: Transcribed text streams directly into your active text field as the AI generates it, giving you zero perceived latency.

## 🧠 How it Works

```mermaid
graph TD
    A[Hold ALT+SPACE] -->|Rust Audio Capture| B(Record Mic)
    B -->|Release Hotkey| C(Local Whisper HTTP Server)
    C -->|Raw Transcript| D{Native Focus Detection API}
    D -->|Context: Slack/Code/Notes| E[FastAPI Python Service]
    E -->|Gemini Flash Lite Cleanup| F(Cleaned Text Stream)
    F -->|Enigo Keystroke Injection| G[Types Directly Into Your App]
```

## 🚀 Quick Start

SpeakType is split into two lightweight, decoupled services.

### 1. Build and Run the Whisper Server
First, build the local inference engine and start the background HTTP server:
```bash
./scripts/setup_whisper.sh

# Leave this running in a background terminal!
./whisper.cpp/build/bin/whisper-server -m ./whisper.cpp/models/ggml-small.en.bin -l en
```

### 2. Start the AI Cleanup Service
This runs the lightweight Python FastAPI server that handles formatting and context-routing.
```bash
cd cleanup_service
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

# Configure your LLM Provider (Gemini or local Ollama)
cp .env.example .env
# Edit .env and add your GEMINI_API_KEY, or set LLM_PROVIDER=ollama

# Run the server
uvicorn main:app --host 127.0.0.1 --port 8008
```

### 3. Run the Rust Daemon
The core daemon handles global hotkeys, audio capture, and keystroke injection.
```bash
cd core
cp config.toml.example config.toml # Modify hotkey here if needed
cargo build --release
./target/release/speaktype
```

## 🎯 Usage
Once both services are running:
1. Click into any text field in any application (Slack, Chrome, VS Code, etc.)
2. Hold `ALT + SPACE`
3. Speak normally
4. Release the keys. Your text will instantly stream into the field perfectly formatted!

### 4. Logs and Troubleshooting
The Rust daemon writes daily rolling logs to `core/logs/speaktype.log.*`. If the daemon fails to start or a hotkey press is ignored, check these logs for detailed error messages.

---
*Built with Rust, Python, and a lot of caffeine.*
