"""
Local cleanup microservice.

Runs on 127.0.0.1:8008 by default. The Rust core POSTs the raw Whisper
transcript + focused-app context here after every dictation and gets back
cleaned, tone-adjusted text ready for injection.

Run:
    uvicorn main:app --host 127.0.0.1 --port 8008
"""
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from dotenv import load_dotenv

load_dotenv()

from graph import cleanup_graph

app = FastAPI(title="speaktype-cleanup")


class CleanupRequest(BaseModel):
    raw_transcript: str
    app_context: str = "unknown"


class CleanupResponse(BaseModel):
    cleaned_text: str


@app.get("/health")
def health():
    return {"status": "ok"}


from fastapi.responses import StreamingResponse

@app.post("/cleanup")
def cleanup_endpoint(req: CleanupRequest):
    if not req.raw_transcript.strip():
        raise HTTPException(status_code=400, detail="raw_transcript is empty")
    
    try:
        stream_iter = cleanup_graph.stream({
            "raw_transcript": req.raw_transcript,
            "app_context": req.app_context,
        })
        first_chunk = next(stream_iter, None)
    except Exception as e:
        raise HTTPException(status_code=502, detail=f"cleanup failed: {e}") from e

    def chunk_generator():
        if first_chunk is not None:
            yield first_chunk
        try:
            for chunk in stream_iter:
                yield chunk
        except Exception as e:
            print(f"Stream error: {e}")

    return StreamingResponse(chunk_generator(), media_type="text/plain")
