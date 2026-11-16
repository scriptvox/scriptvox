"""One transcript, one process: `python -m scriptvox_whisper in.wav out.json [--lang ar] [--prompt text]`.

The JSON has the shape of the self-hosted row (`services/gen/engines_audio.py`): language, text, segments, words.
"""

from __future__ import annotations

import argparse
import json
import os
import re
from pathlib import Path

# Without a short punctuated prompt, Whisper often writes a clip with no punctuation (seen 3 Nov on the self-hosted row).
PROMPTS = {
    "en": "Hello, and welcome. Today, we look at the city.",
    "ar": "مرحباً بكم. اليوم، نلقي نظرة على المدينة.",
}


def guard_alignment(model):
    """faster-whisper 1.2.1: a tiny last window can align to nothing, and `find_alignment` then raises IndexError. The
    same file fails on every retry (seen 5 Nov on one 30.0 s clip, large-v3, also on CPU). That window has no word times:
    it is a false repeat past the end of the audio, and the caller drops it."""
    real = model.find_alignment

    def guarded(tokenizer, text_tokens, *args, **kwargs):
        try:
            return real(tokenizer, text_tokens, *args, **kwargs)
        except IndexError:
            return [[] for _ in text_tokens]

    model.find_alignment = guarded
    return model


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("wav")
    p.add_argument("out")
    p.add_argument("--lang")
    p.add_argument("--prompt")
    a = p.parse_args()

    from faster_whisper import WhisperModel

    model = guard_alignment(WhisperModel(os.environ["SCRIPTVOX_WHISPER_MODEL"], device="cpu", compute_type="int8"))
    prompt = a.prompt or PROMPTS.get(a.lang or "", PROMPTS["en"])
    prompt_words = set(re.findall(r"\w+", prompt.lower()))
    segs, info = model.transcribe(
        a.wav, language=a.lang, word_timestamps=True, vad_filter=True, beam_size=5, initial_prompt=prompt
    )
    segments, words = [], []
    for s in segs:
        # No word times: the false repeat that `guard_alignment` lets through.
        if not s.words:
            continue
        # On music or noise, Whisper may write its own prompt back.
        said = re.findall(r"\w+", s.text.lower())
        if len(said) >= 4 and s.no_speech_prob > 0.3 and sum(w in prompt_words for w in said) / len(said) >= 0.8:
            continue
        segments.append({"start": round(s.start, 3), "end": round(s.end, 3), "text": s.text.strip()})
        for w in s.words or []:
            words.append({"text": w.word.strip(), "start": round(w.start, 3), "end": round(w.end, 3)})
    doc = {
        "language": info.language,
        "duration": round(info.duration, 3),
        "text": " ".join(s["text"] for s in segments),
        "segments": segments,
        "words": words,
    }
    Path(a.out).write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
