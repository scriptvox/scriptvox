"""Sound engines for scriptvox-gen: text to speech (Chatterbox Multilingual), speech to text (faster-whisper), and sound
effects that follow a video (MMAudio, testing only: its weights are CC BY-NC 4.0).

Both read weights from MODELS_DIR (relayed, no internet). Audio input arrives as a data URI and is decoded to a
temporary file; nothing is written outside OUT_DIR and the temp folder.
"""

import base64
import json
import os
import re
import tempfile
from pathlib import Path

MODELS = Path(os.environ.get("MODELS_DIR", "/models"))
_loaded: dict[str, object] = {}
PROMPTS = {
    "en": "Hello, and welcome. Today, we look at the city.",
    "ar": "مرحباً بكم. اليوم، نلقي نظرة على المدينة.",
}
DATA_URI = re.compile(r"^data:audio/[a-z0-9.+-]+;base64,(.+)$", re.DOTALL)


def decode_audio(uri: str, folder: str) -> str:
    m = DATA_URI.match(uri or "")
    if not m:
        raise ValueError("audio must be a data:audio/...;base64 URI")
    p = Path(folder) / "in.audio"
    p.write_bytes(base64.b64decode(m.group(1)))
    return str(p)


class ChatterboxTTS:
    """Chatterbox Multilingual (MIT): 23 languages incl. Arabic and English; clones a voice from a short sample.
    Output carries the model's inaudible Perth watermark."""

    folder = "chatterbox"
    # The 23 codes of chatterbox 0.1.7 (mtl_tts.SUPPORTED_LANGUAGES).
    langs = frozenset({
        "ar", "da", "de", "el", "en", "es", "fi", "fr", "he", "hi", "it", "ja",
        "ko", "ms", "nl", "no", "pl", "pt", "ru", "sv", "sw", "tr", "zh",
    })

    def model(self):
        if "tts" not in _loaded:
            from chatterbox.mtl_tts import ChatterboxMultilingualTTS

            _loaded["tts"] = ChatterboxMultilingualTTS.from_local(MODELS / self.folder, device="cuda")
        return _loaded["tts"]

    def warm(self):
        self.model()

    def validate(self, inp: dict) -> str | None:
        t = inp.get("text")
        if not isinstance(t, str) or not t.strip() or len(t) > 2000:
            return "text must be 1 to 2000 characters (split long scripts by sentence)"
        if inp.get("language", "ar") not in self.langs:
            return f"language must be one of {sorted(self.langs)}"
        if inp.get("reference_audio_url") and not DATA_URI.match(inp["reference_audio_url"]):
            return "reference_audio_url must be a data:audio URI"
        return None

    def run(self, inp: dict, out_base: Path):
        import torchaudio

        m = self.model()
        with tempfile.TemporaryDirectory() as tmp:
            ref = decode_audio(inp["reference_audio_url"], tmp) if inp.get("reference_audio_url") else None
            wav = m.generate(
                inp["text"],
                language_id=inp.get("language", "ar"),
                audio_prompt_path=ref,
                exaggeration=float(inp.get("exaggeration", 0.5)),
                cfg_weight=float(inp.get("cfg_weight", 0.5)),
            )
        out = out_base.with_suffix(".wav")
        torchaudio.save(str(out), wav, m.sr)
        return out, {"sample_rate": m.sr, "seconds": round(wav.shape[-1] / m.sr, 2), "cloned": ref is not None}


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


class WhisperSTT:
    """faster-whisper large-v3 (MIT): Arabic and English, word timestamps, built-in VAD."""

    folder = "faster-whisper-large-v3"

    def model(self):
        if "stt" not in _loaded:
            from faster_whisper import WhisperModel

            _loaded["stt"] = guard_alignment(
                WhisperModel(str(MODELS / self.folder), device="cuda", compute_type="float16")
            )
        return _loaded["stt"]

    def warm(self):
        self.model()

    def validate(self, inp: dict) -> str | None:
        if not DATA_URI.match(inp.get("audio_url") or ""):
            return "audio_url must be a data:audio URI"
        return None

    def run(self, inp: dict, out_base: Path):
        with tempfile.TemporaryDirectory() as tmp:
            src = decode_audio(inp["audio_url"], tmp)
            # A short punctuated prompt: without one, Whisper often writes a clip that starts mid-sentence in lower
            # case with no punctuation (seen 3 Nov on `edit --captions`). Language detection does not use it.
            lang = inp.get("language")
            prompt = inp.get("prompt") or PROMPTS.get(lang or "", PROMPTS["en"])
            segs, info = self.model().transcribe(
                src, language=lang, word_timestamps=True, vad_filter=True, beam_size=5, initial_prompt=prompt
            )
            segments, words = [], []
            prompt_words = set(re.findall(r"\w+", prompt.lower()))
            for s in segs:
                # No word times: the false repeat that `guard_alignment` lets through.
                if not s.words:
                    continue
                # On music or noise, Whisper may write its own prompt back, or a likely sentence, with a high
                # no-speech score (seen 3 Nov: "So, let's take a look at the city." on a voice-free stem).
                said = re.findall(r"\w+", s.text.lower())
                # Real speech that shares the prompt's words keeps a low no-speech score, so both must agree.
                echo = (len(said) >= 4 and s.no_speech_prob > 0.3
                        and sum(w in prompt_words for w in said) / len(said) >= 0.8)
                # No rule on the scores alone: measured 3 Nov, a hallucination on a voice-free stem scored 0.505 and
                # -0.80, but real repeated speech scored the same (no-speech 0.47, fallback decodes under -0.6), and
                # a score rule dropped 97 of 120 real words. The scores stay in the output for the caller.
                if echo:
                    continue
                segments.append({"start": round(s.start, 3), "end": round(s.end, 3), "text": s.text.strip(),
                                 "no_speech": round(s.no_speech_prob, 3), "logprob": round(s.avg_logprob, 3)})
                for w in s.words or []:
                    words.append({"text": w.word.strip(), "start": round(w.start, 3), "end": round(w.end, 3),
                                  "p": round(w.probability, 3)})
        doc = {"language": info.language, "language_p": round(info.language_probability, 3),
               "duration": round(info.duration, 3), "text": " ".join(s["text"] for s in segments),
               "segments": segments, "words": words}
        out = out_base.with_suffix(".json")
        out.write_text(json.dumps(doc, ensure_ascii=False))
        return out, {"words": len(words), "language": info.language, "seconds_audio": doc["duration"]}


class MMAudioFoley:
    """MMAudio large 44k v2 (code MIT, weights CC BY-NC 4.0, so testing only): sound that follows a video (v2a), or
    from text alone (t2a). Trained on 8 s clips; up to 10 s here. Weights in MODELS_DIR/mmaudio: weights/,
    ext_weights/ (VAE, Synchformer) and hf/ (a Hugging Face cache with the CLIP and BigVGAN repos, read offline)."""

    folder = "mmaudio"

    def __init__(self, video: bool):
        self.video = video

    def model(self):
        if "foley" not in _loaded:
            import torch
            from mmaudio.eval_utils import ModelConfig
            from mmaudio.model.flow_matching import FlowMatching
            from mmaudio.model.networks import get_my_mmaudio
            from mmaudio.model.utils.features_utils import FeaturesUtils

            d = MODELS / self.folder
            cfg = ModelConfig(model_name="large_44k_v2", model_path=d / "weights/mmaudio_large_44k_v2.pth",
                              vae_path=d / "ext_weights/v1-44.pth", bigvgan_16k_path=None, mode="44k",
                              synchformer_ckpt=d / "ext_weights/synchformer_state_dict.pth")
            dt = torch.bfloat16
            net = get_my_mmaudio(cfg.model_name).to("cuda", dt).eval()
            net.load_weights(torch.load(cfg.model_path, map_location="cuda", weights_only=True))
            fu = FeaturesUtils(tod_vae_ckpt=str(cfg.vae_path), synchformer_ckpt=str(cfg.synchformer_ckpt),
                               enable_conditions=True, mode="44k", bigvgan_vocoder_ckpt=None,
                               need_vae_encoder=False).to("cuda", dt).eval()
            _loaded["foley"] = (cfg, net, fu, FlowMatching)
        return _loaded["foley"]

    def warm(self):
        self.model()

    def validate(self, inp: dict) -> str | None:
        if self.video and not re.match(r"^data:video/mp4;base64,", str(inp.get("video_url", ""))):
            return "video_url must be a data:video/mp4;base64 URI"
        if not self.video and not str(inp.get("prompt", "")).strip():
            return "prompt is required for text to audio"
        if len(str(inp.get("prompt", ""))) > 500:
            return "prompt must be at most 500 characters"
        s = inp.get("duration", 8)
        if not isinstance(s, (int, float)) or not 1 <= s <= 10:
            return "duration must be 1 to 10 seconds"
        return None

    def run(self, inp: dict, out_base: Path):
        import torch
        import torchaudio
        from mmaudio.eval_utils import generate, load_video

        cfg, net, fu, flow = self.model()
        secs = float(inp.get("duration", 8))
        clip = sync = None
        with tempfile.TemporaryDirectory() as tmp:
            if self.video:
                src = Path(tmp) / "in.mp4"
                src.write_bytes(base64.b64decode(inp["video_url"].split(",", 1)[1]))
                info = load_video(src, secs, load_all_frames=False)
                clip, sync = info.clip_frames.unsqueeze(0), info.sync_frames.unsqueeze(0)
                secs = info.duration_sec
        seq = cfg.seq_cfg
        seq.duration = secs
        net.update_seq_lengths(seq.latent_seq_len, seq.clip_seq_len, seq.sync_seq_len)
        rng = torch.Generator(device="cuda")
        rng.manual_seed(int(inp.get("seed", 42)))
        with torch.inference_mode():
            audio = generate(clip, sync, [str(inp.get("prompt", ""))],
                             negative_text=[str(inp.get("negative_prompt", "music, speech, voice"))],
                             feature_utils=fu, net=net, fm=flow(min_sigma=0, inference_mode="euler",
                                                                 num_steps=int(inp.get("steps", 25))),
                             rng=rng, cfg_strength=float(inp.get("cfg_strength", 4.5)))
        wav = audio.float().cpu()[0]
        out = out_base.with_suffix(".wav")
        torchaudio.save(str(out), wav, seq.sampling_rate)
        return out, {"sample_rate": seq.sampling_rate, "seconds": round(secs, 2), "from_video": self.video}


class DemucsStems:
    """Demucs v4 htdemucs (MIT, Meta): splits a track into voice and the rest. Returns the rest (music, ambience,
    effects) so a dub keeps the original background under the new voice. Weights: MODELS_DIR/demucs (htdemucs.yaml
    and the 955717e8 checkpoint)."""

    folder = "demucs"

    def model(self):
        if "stems" not in _loaded:
            import hashlib

            import torch
            from demucs.states import load_model

            # Meta's own checkpoint: its name carries the start of its SHA-256, checked here before loading. It
            # pickles the model class, so torch 2.6's weights_only default must be turned off for this one file.
            p = MODELS / self.folder / "955717e8-8726e21a.th"
            if not hashlib.sha256(p.read_bytes()).hexdigest().startswith("8726e21a"):
                raise ValueError(f"{p.name} does not match its checksum")
            m = load_model(torch.load(p, map_location="cpu", weights_only=False))
            m.to("cuda").eval()
            _loaded["stems"] = m
        return _loaded["stems"]

    def warm(self):
        self.model()

    def validate(self, inp: dict) -> str | None:
        if not DATA_URI.match(inp.get("audio_url") or ""):
            return "audio_url must be a data:audio URI"
        return None

    def run(self, inp: dict, out_base: Path):
        import torch
        import torchaudio
        from demucs.apply import apply_model

        m = self.model()
        with tempfile.TemporaryDirectory() as tmp:
            wav, sr = torchaudio.load(decode_audio(inp["audio_url"], tmp))
        if sr != m.samplerate:
            wav = torchaudio.functional.resample(wav, sr, m.samplerate)
        wav = wav[:2] if wav.shape[0] >= 2 else wav.repeat(2, 1)
        ref = wav.mean(0)
        mean, std = ref.mean(), ref.std() + 1e-8
        with torch.inference_mode():
            src = apply_model(m, ((wav - mean) / std)[None].cuda(), split=True, overlap=0.25)[0]
        src = src.cpu() * std + mean
        voice = src[m.sources.index("vocals")]
        rest = src.sum(0) - voice
        out = out_base.with_suffix(".wav")
        torchaudio.save(str(out), rest, m.samplerate)
        level = lambda x: round(float(20 * torch.log10(x.pow(2).mean().sqrt() + 1e-9)), 1)
        return out, {"seconds": round(rest.shape[-1] / m.samplerate, 2), "voice_db": level(voice),
                     "rest_db": level(rest)}


ENGINES = {
    "demucs-htdemucs/stems": DemucsStems(),
    "chatterbox-mtl/tts": ChatterboxTTS(),
    "whisper-large-v3/stt": WhisperSTT(),
    "mmaudio-large-44k/v2a": MMAudioFoley(video=True),
    "mmaudio-large-44k/t2a": MMAudioFoley(video=False),
}
