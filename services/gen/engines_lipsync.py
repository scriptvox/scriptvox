"""Lip sync engine for scriptvox-gen: LatentSync 1.6 (ByteDance). A video and a new voice in, the same video with the
mouth moving to that voice out. Audio-driven, so any language. It runs in its own image, on its own GPU.

Licences stay with their owners: LatentSync code Apache-2.0, weights OpenRAIL++ (use limits), InsightFace buffalo_l
face models non-commercial research only. ScriptVox runs them as a separate part and records this on its row.
Weights: MODELS_DIR/latentsync-16 (relayed, no internet), seen by the code as /opt/latentsync/checkpoints.
"""

import base64
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

CODE = "/opt/latentsync"
VIDEO = re.compile(r"^data:video/mp4;base64,(.+)$", re.DOTALL)
AUDIO = re.compile(r"^data:audio/[a-z0-9.+-]+;base64,(.+)$", re.DOTALL)
_loaded: dict[str, object] = {}


class LatentSync:
    def model(self):
        if "pipe" not in _loaded:
            import torch
            from diffusers import AutoencoderKL, DDIMScheduler
            from latentsync.models.unet import UNet3DConditionModel
            from latentsync.pipelines.lipsync_pipeline import LipsyncPipeline
            from latentsync.whisper.audio2feature import Audio2Feature
            from omegaconf import OmegaConf

            os.chdir(CODE)  # the project reads its configs and mask by relative path
            config = OmegaConf.load("configs/unet/stage2_512.yaml")
            audio = Audio2Feature(
                model_path="checkpoints/whisper/tiny.pt",
                device="cuda",
                num_frames=config.data.num_frames,
                audio_feat_length=config.data.audio_feat_length,
            )
            vae = AutoencoderKL.from_pretrained("checkpoints/vae", torch_dtype=torch.float16)
            vae.config.scaling_factor = 0.18215
            vae.config.shift_factor = 0
            unet, _ = UNet3DConditionModel.from_pretrained(
                OmegaConf.to_container(config.model), "checkpoints/latentsync_unet.pt", device="cpu"
            )
            pipe = LipsyncPipeline(
                vae=vae,
                audio_encoder=audio,
                unet=unet.to(dtype=torch.float16),
                scheduler=DDIMScheduler.from_pretrained("configs"),
            ).to("cuda")
            _loaded.update(pipe=pipe, config=config)
        return _loaded["pipe"], _loaded["config"]

    def warm(self):
        self.model()

    def validate(self, inp: dict) -> str | None:
        if not VIDEO.match(inp.get("video_url") or ""):
            return "video_url must be a data:video/mp4 URI"
        if not AUDIO.match(inp.get("audio_url") or ""):
            return "audio_url must be a data:audio URI"
        return None

    def run(self, inp: dict, out_base: Path):
        import torch

        pipe, config = self.model()
        out = out_base.with_suffix(".mp4")
        with tempfile.TemporaryDirectory() as tmp:
            video, audio = Path(tmp) / "in.mp4", Path(tmp) / "in.audio"
            video.write_bytes(base64.b64decode(VIDEO.match(inp["video_url"]).group(1)))
            audio.write_bytes(base64.b64decode(AUDIO.match(inp["audio_url"]).group(1)))
            wav = Path(tmp) / "in.wav"
            subprocess.run(["ffmpeg", "-nostdin", "-v", "error", "-y", "-i", str(audio), "-ac", "1", "-ar", "16000", str(wav)], check=True)
            work = Path(tmp) / "work"
            pipe(
                video_path=str(video),
                audio_path=str(wav),
                video_out_path=str(out),
                num_frames=config.data.num_frames,
                num_inference_steps=int(inp.get("steps", 20)),
                guidance_scale=float(inp.get("guidance", 1.5)),
                weight_dtype=torch.float16,
                width=config.data.resolution,
                height=config.data.resolution,
                mask_image_path=config.data.mask_image_path,
                temp_dir=str(work),
            )
            shutil.rmtree(work, ignore_errors=True)
        if not out.is_file():
            raise RuntimeError("the pipeline wrote no video")
        return out, {"steps": int(inp.get("steps", 20))}


ENGINES = {"latentsync-16/lipsync": LatentSync()}
