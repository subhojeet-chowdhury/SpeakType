#!/usr/bin/env bash
set -e

echo "🎙️ Starting SpeakType Startup Sequence..."

# 1. Setup / Check Whisper
if [ ! -f "./whisper.cpp/build/bin/whisper-server" ]; then
    echo "Whisper server not found. Running setup script..."
    ./scripts/setup_whisper.sh
fi

# 2. Setup / Check Cleanup Service
cd cleanup_service
if [ ! -d ".venv" ]; then
    echo "Python virtual environment not found. Creating..."
    python3 -m venv .venv
    source .venv/bin/activate
    pip install -r requirements.txt
else
    source .venv/bin/activate
fi
cd ..

# 3. Build Rust Core (only builds if there are changes)
echo "Building Rust daemon..."
cd core
cargo build --release
cd ..

# 4. Create logs directory
mkdir -p logs

# 5. Process management (trap)
cleanup() {
    echo ""
    echo "🛑 Stopping SpeakType services..."
    # Kill background jobs spawned by this script
    kill $(jobs -p) 2>/dev/null || true
    echo "✅ SpeakType stopped."
    exit
}
trap cleanup SIGINT SIGTERM

echo ""
echo "🚀 Booting services..."

# Start Whisper Server in background
echo "-> Starting Whisper HTTP Server (port 8080)... logs in logs/whisper.log"
./whisper.cpp/build/bin/whisper-server -m ./whisper.cpp/models/ggml-base.en.bin -l en > logs/whisper.log 2>&1 &

# Start Cleanup Service in background
echo "-> Starting FastAPI Cleanup Service (port 8008)... logs in logs/cleanup.log"
cd cleanup_service
# We already activated .venv above
uvicorn main:app --host 127.0.0.1 --port 8008 > ../logs/cleanup.log 2>&1 &
cd ..

# Give the background servers a second to bind to their ports
echo "-> Waiting for servers to initialize..."
sleep 2

# Start Rust Daemon in foreground
echo "-> Starting Rust Core Daemon..."
./core/target/release/speaktype
