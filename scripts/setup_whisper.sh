#!/usr/bin/env bash
# Clones and builds whisper.cpp, and downloads the base.en ggml model.
# Run this once, from the repo root, before running the core daemon.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ ! -d whisper.cpp ]; then
    git clone https://github.com/ggerganov/whisper.cpp.git
fi

cd whisper.cpp
cmake -B build -DWHISPER_BUILD_SERVER=ON
cmake --build build -j --config Release --target server

# base.en is a good starting point: ~140MB, decent accuracy, fast on CPU.
# Swap for small.en/medium.en later if you want higher accuracy and can
# tolerate more latency + RAM.

# for base model
# bash ./models/download-ggml-model.sh base.en

# for small model
bash ./models/download-ggml-model.sh small.en

echo ""
echo "Done. The Whisper server was built successfully!"
echo "Before running the SpeakType daemon, you MUST start the whisper server in the background:"
echo ""
echo "    ./whisper.cpp/build/bin/whisper-server -m ./whisper.cpp/models/ggml-small.en.bin -l en"
echo ""
