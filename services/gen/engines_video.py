"""Video and image engines for scriptvox-gen (diffusers). One model stays on the GPU; asking for another frees it."""

import os
import re
from pathlib import Path

MODELS = Path(os.environ.get("MODELS_DIR", "/models"))
STEPS = int(os.environ.get("STEPS", "40"))
FPS = 24
# (width, height) per aspect; Wan 2.2 TI2V 5B is trained at 1280x704 (both sides divisible by 32).
WAN_SIZES = {"9:16": (704, 1280), "16:9": (1280, 704), "1:1": (960, 960)}
IMAGE_SIZES = {"9:16": (768, 1344), "16:9": (1344, 768), "1:1": (1024, 1024)}
NEGATIVE = (
    "blurry, low quality, distorted faces, extra limbs, warped hands, text, watermark, logo, subtitles, "
    "flicker, jitter, static frame, overexposed"
)

_loaded: dict[str, object] = {}


def resident(name: str, load):
    """Keeps one pipeline on the GPU."""
    if name not in _loaded:
        import torch

        _loaded.clear()
        torch.cuda.empty_cache()
        _loaded[name] = load()
    return _loaded[name]


def check_prompt(inp: dict, sizes: dict) -> str | None:
    if not isinstance(inp.get("prompt"), str) or not inp["prompt"].strip() or len(inp["prompt"]) > 4000:
        return "prompt must be text of 1 to 4000 characters"
    if inp.get("aspect_ratio", "9:16") not in sizes:
        return f"aspect_ratio must be one of {sorted(sizes)}"
    return None


class WanT2V:
    """Wan 2.2 TI2V 5B, text to video (Apache-2.0)."""

    folder = "wan22-ti2v-5b"

    def pipe(self):
        def load():
            import torch
            from diffusers import AutoencoderKLWan, WanPipeline

            path = MODELS / self.folder
            vae = AutoencoderKLWan.from_pretrained(path, subfolder="vae", torch_dtype=torch.float32)
            return WanPipeline.from_pretrained(path, vae=vae, torch_dtype=torch.bfloat16).to("cuda")

        return resident(self.folder, load)

    def warm(self):
        self.pipe()

    def validate(self, inp: dict) -> str | None:
        bad = check_prompt(inp, WAN_SIZES)
        if bad:
            return bad
        try:
            if not 1 <= float(inp.get("duration", 5)) <= 10:
                return "duration must be 1 to 10 seconds"
        except (TypeError, ValueError):
            return "duration must be a number of seconds"
        return None

    def run(self, inp: dict, out_base: Path):
        from diffusers.utils import export_to_video

        w, h = WAN_SIZES[inp.get("aspect_ratio", "9:16")]
        frames = 4 * max(1, round(float(inp.get("duration", 5)) * FPS / 4)) + 1  # Wan wants 4k+1 frames
        out = self.pipe()(
            prompt=inp["prompt"],
            negative_prompt=inp.get("negative_prompt", NEGATIVE),
            width=w,
            height=h,
            num_frames=frames,
            guidance_scale=float(inp.get("guidance_scale", 5.0)),
            num_inference_steps=STEPS,
        ).frames[0]
        part = out_base.with_suffix(".part.mp4")
        export_to_video(out, str(part), fps=FPS)
        final = out_base.with_suffix(".mp4")
        part.replace(final)
        return final, {"width": w, "height": h, "steps": STEPS}


def decode_image(uri: str):
    import base64
    import io
    import re

    from PIL import Image

    m = re.match(r"^data:image/[a-z0-9.+-]+;base64,(.+)$", uri or "", re.DOTALL)
    if not m:
        raise ValueError("image_url must be a data:image/...;base64 URI")
    return Image.open(io.BytesIO(base64.b64decode(m.group(1)))).convert("RGB")


def cover(img, w: int, h: int):
    """Scale to cover w x h and centre crop, so the first frame fills the clip with no bars."""
    s = max(w / img.width, h / img.height)
    img = img.resize((round(img.width * s), round(img.height * s)))
    left, top = (img.width - w) // 2, (img.height - h) // 2
    return img.crop((left, top, left + w, top + h))


class WanI2V(WanT2V):
    """Wan 2.2 TI2V 5B, image to video: the same weights, the image is the first frame (diffusers
    WanImageToVideoPipeline; the 5B checkpoint needs no CLIP image encoder)."""

    def pipe(self):
        def load():
            import torch
            from diffusers import AutoencoderKLWan, WanImageToVideoPipeline

            path = MODELS / self.folder
            vae = AutoencoderKLWan.from_pretrained(path, subfolder="vae", torch_dtype=torch.float32)
            return WanImageToVideoPipeline.from_pretrained(path, vae=vae, torch_dtype=torch.bfloat16).to("cuda")

        return resident(self.folder + "-i2v", load)

    def validate(self, inp: dict) -> str | None:
        if not str(inp.get("image_url", "")).startswith("data:image/"):
            return "image_url must be a data:image URI (the first frame)"
        return super().validate(inp)

    def run(self, inp: dict, out_base: Path):
        from diffusers.utils import export_to_video

        w, h = WAN_SIZES[inp.get("aspect_ratio", "9:16")]
        frames = 4 * max(1, round(float(inp.get("duration", 5)) * FPS / 4)) + 1
        out = self.pipe()(
            image=cover(decode_image(inp["image_url"]), w, h),
            prompt=inp["prompt"],
            negative_prompt=inp.get("negative_prompt", NEGATIVE),
            width=w,
            height=h,
            num_frames=frames,
            guidance_scale=float(inp.get("guidance_scale", 5.0)),
            num_inference_steps=STEPS,
        ).frames[0]
        part = out_base.with_suffix(".part.mp4")
        export_to_video(out, str(part), fps=FPS)
        final = out_base.with_suffix(".mp4")
        part.replace(final)
        return final, {"width": w, "height": h, "steps": STEPS}


def dmd_scheduler(base, steps=(1000, 757, 522)):
    """FastWan was distilled with DMD: at each of its few timesteps it predicts the clean latents, and the next step
    starts from those latents noised again to the next timestep (not an Euler step). Same interface as the flow
    matching scheduler the Wan pipelines call."""
    import torch
    from diffusers import FlowMatchEulerDiscreteScheduler

    class DMD(FlowMatchEulerDiscreteScheduler):
        def set_timesteps(self, num_inference_steps=None, device=None, **kw):
            self.timesteps = torch.tensor(steps, dtype=torch.float32, device=device)
            self.sigmas = torch.tensor([*(t / 1000 for t in steps), 0.0], dtype=torch.float32, device=device)
            self.num_inference_steps = len(steps)
            self._step_index = None
            self._begin_index = None

        def step(self, model_output, timestep, sample, return_dict=True, generator=None, **kw):
            t = float(timestep.flatten()[0]) if hasattr(timestep, "flatten") else float(timestep)
            i = min(range(len(steps)), key=lambda k: abs(steps[k] - t))
            x0 = sample.float() - (t / 1000) * model_output.float()
            nxt = steps[i + 1] / 1000 if i + 1 < len(steps) else 0.0
            if nxt > 0:
                noise = torch.randn(x0.shape, generator=generator, device=x0.device, dtype=x0.dtype)
                x0 = (1 - nxt) * x0 + nxt * noise
            out = x0.to(sample.dtype)
            return (out,) if not return_dict else type("Out", (), {"prev_sample": out})()

    return DMD.from_config(base.config)


class FastWanT2V(WanT2V):
    """FastWan 2.2 TI2V 5B (FastVideo, Apache-2.0): Wan 2.2 TI2V 5B distilled to 3 steps with DMD. Only the
    transformer differs; the text encoder and VAE come from the base model folder."""

    fast = "fastwan22-ti2v-5b"
    pipeline = "WanPipeline"

    def pipe(self):
        def load():
            import diffusers
            import torch
            from diffusers import AutoencoderKLWan, WanTransformer3DModel

            path = MODELS / self.folder
            vae = AutoencoderKLWan.from_pretrained(path, subfolder="vae", torch_dtype=torch.float32)
            tr = WanTransformer3DModel.from_pretrained(MODELS / self.fast, subfolder="transformer",
                                                       torch_dtype=torch.bfloat16)
            p = getattr(diffusers, self.pipeline).from_pretrained(path, vae=vae, transformer=tr,
                                                                  torch_dtype=torch.bfloat16)
            p.scheduler = dmd_scheduler(p.scheduler)
            return p.to("cuda")

        return resident(f"{self.fast}-{self.pipeline}", load)

    def run(self, inp: dict, out_base: Path):
        from diffusers.utils import export_to_video

        w, h = WAN_SIZES[inp.get("aspect_ratio", "9:16")]
        frames = 4 * max(1, round(float(inp.get("duration", 5)) * FPS / 4)) + 1
        args = {"prompt": inp["prompt"], "width": w, "height": h, "num_frames": frames,
                "guidance_scale": 1.0, "num_inference_steps": 3}
        if inp.get("image_url"):
            args["image"] = cover(decode_image(inp["image_url"]), w, h)
        out = self.pipe()(**args).frames[0]
        part = out_base.with_suffix(".part.mp4")
        export_to_video(out, str(part), fps=FPS)
        final = out_base.with_suffix(".mp4")
        part.replace(final)
        return final, {"width": w, "height": h, "steps": 3}


class FastWanI2V(FastWanT2V):
    """FastWan, image to video: the image is the first frame."""

    pipeline = "WanImageToVideoPipeline"

    def validate(self, inp: dict) -> str | None:
        if not str(inp.get("image_url", "")).startswith("data:image/"):
            return "image_url must be a data:image URI (the first frame)"
        return super().validate(inp)


class FluxKlein:
    """FLUX.2 klein 4B (Apache-2.0), text to image, 4 steps (model card settings)."""

    folder = "flux2-klein-4b"

    def pipe(self):
        def load():
            import torch
            from diffusers import Flux2KleinPipeline

            return Flux2KleinPipeline.from_pretrained(MODELS / self.folder, torch_dtype=torch.bfloat16).to("cuda")

        return resident(self.folder, load)

    def warm(self):
        self.pipe()

    def validate(self, inp: dict) -> str | None:
        refs = inp.get("image_urls", [])
        if not isinstance(refs, list) or len(refs) > 4 or not all(str(u).startswith("data:image/") for u in refs):
            return "image_urls must be at most 4 data:image URIs (reference pictures)"
        return check_prompt(inp, IMAGE_SIZES)

    def run(self, inp: dict, out_base: Path):
        w, h = IMAGE_SIZES[inp.get("aspect_ratio", "9:16")]
        refs = [decode_image(u) for u in inp.get("image_urls", [])]
        img = self.pipe()(
            prompt=inp["prompt"],
            image=refs or None,
            width=w,
            height=h,
            num_inference_steps=4,
            guidance_scale=1.0,
        ).images[0]
        out = out_base.with_suffix(".png")
        img.save(out)
        return out, {"width": w, "height": h, "steps": 4}


class AceStepMusic:
    """ACE-Step v1.5 XL Turbo (MIT), text to music, 8 steps (guidance-distilled), stereo 48 kHz. Empty lyrics give
    an instrumental bed, which is what scriptvox puts under a voice."""

    folder = "acestep-xl-turbo"

    def pipe(self):
        def load():
            import torch
            from diffusers import AceStepPipeline

            return AceStepPipeline.from_pretrained(MODELS / self.folder, torch_dtype=torch.bfloat16).to("cuda")

        return resident(self.folder, load)

    def warm(self):
        self.pipe()

    def validate(self, inp: dict) -> str | None:
        p = inp.get("prompt")
        if not isinstance(p, str) or not p.strip() or len(p) > 1000:
            return "prompt must be text of 1 to 1000 characters (style, instruments, mood, tempo)"
        try:
            if not 5 <= float(inp.get("duration", 30)) <= 240:
                return "duration must be 5 to 240 seconds"
        except (TypeError, ValueError):
            return "duration must be a number of seconds"
        return None

    def run(self, inp: dict, out_base: Path):
        import wave

        import numpy as np

        pipe = self.pipe()
        audio = pipe(
            prompt=inp["prompt"],
            lyrics=inp.get("lyrics", ""),
            audio_duration=float(inp.get("duration", 30)),
            num_inference_steps=8,
        ).audios[0]
        pcm = (np.clip(audio.float().cpu().numpy(), -1.0, 1.0) * 32767).astype("<i2")  # (channels, samples)
        out = out_base.with_suffix(".wav")
        with wave.open(str(out), "wb") as w:
            w.setnchannels(pcm.shape[0])
            w.setsampwidth(2)
            w.setframerate(pipe.sample_rate)
            w.writeframes(pcm.T.tobytes())
        return out, {"sample_rate": pipe.sample_rate, "seconds": round(pcm.shape[1] / pipe.sample_rate, 2)}


def free_gpu():
    """Frees the resident pipeline, for an engine that runs in its own process."""
    import torch

    _loaded.clear()
    torch.cuda.empty_cache()


class SeedVR2Upscale:
    """SeedVR2 3B video restoration and upscale (ByteDance-Seed, Apache-2.0), one step per frame batch, through the
    inference CLI of numz/ComfyUI-SeedVR2_VideoUpscaler v2.5.23 (Apache-2.0) in its own process, so its memory
    goes back when it ends."""

    folder = "seedvr2-3b"
    dit = "seedvr2_ema_3b_fp16.safetensors"
    cli = Path(os.environ.get("SEEDVR2_CLI", "/opt/seedvr2/inference_cli.py"))

    def warm(self):
        pass

    def validate(self, inp: dict) -> str | None:
        if not re.match(r"^data:video/mp4;base64,", str(inp.get("video_url", ""))):
            return "video_url must be a data:video/mp4;base64 URI"
        if int(inp.get("resolution", 1080)) not in (720, 1080, 1440, 2160):
            return "resolution (the short side) must be 720, 1080, 1440 or 2160"
        if (int(inp.get("batch_size", 41)) - 1) % 4:
            return "batch_size must be 4n+1 (5, 9, 13, ... 41, ...)"
        return None

    def run(self, inp: dict, out_base: Path):
        import base64
        import subprocess
        import tempfile

        free_gpu()
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "in.mp4"
            src.write_bytes(base64.b64decode(inp["video_url"].split(",", 1)[1]))
            part = out_base.with_suffix(".part.mp4")
            cmd = [
                "python", str(self.cli), str(src), "--output", str(part), "--video_backend", "ffmpeg",
                "--model_dir", str(MODELS / self.folder), "--dit_model", self.dit,
                "--resolution", str(int(inp.get("resolution", 1080))),
                "--batch_size", str(int(inp.get("batch_size", 41))),
                "--temporal_overlap", "4", "--color_correction", "lab", "--seed", str(int(inp.get("seed", 42))),
                # Tiled VAE: on an H100 a 121-frame 1080p decode took 153 s whole and 52 s in tiles (PSNR 37 dB apart).
                "--vae_encode_tiled", "--vae_decode_tiled",
            ]
            r = subprocess.run(cmd, cwd=self.cli.parent, capture_output=True, text=True, timeout=3600, check=False)
            if r.returncode or not part.is_file():
                raise RuntimeError(f"seedvr2 exit {r.returncode}: {(r.stderr or r.stdout)[-400:]}")
        final = out_base.with_suffix(".mp4")
        part.replace(final)
        return final, {"resolution": int(inp.get("resolution", 1080)), "batch_size": int(inp.get("batch_size", 41))}


class BiRefNetMatte:
    """BiRefNet (ZhengPeng7, MIT): the main subject's mask on every frame of a clip, as a grey mp4 of the same size
    and frame count (white = subject). The model code ships with the weights (birefnet.py, MIT)."""

    folder = "birefnet"

    def model(self):
        def load():
            import importlib
            import sys

            import torch

            # birefnet.py imports its config relatively: load the weights folder as a namespace package.
            sys.path.insert(0, str(MODELS))
            mod = importlib.import_module(f"{self.folder}.birefnet")
            m = mod.BiRefNet.from_pretrained(str(MODELS / self.folder))
            return m.to("cuda", dtype=torch.float16).eval()

        return resident(self.folder, load)

    def warm(self):
        self.model()

    def validate(self, inp: dict) -> str | None:
        if not re.match(r"^data:video/mp4;base64,", str(inp.get("video_url", ""))):
            return "video_url must be a data:video/mp4;base64 URI"
        return None

    def run(self, inp: dict, out_base: Path):
        import base64
        import tempfile

        import imageio.v2 as iio
        import numpy as np
        import torch
        import torch.nn.functional as F

        model = self.model()
        mean = torch.tensor([0.485, 0.456, 0.406], device="cuda").view(1, 3, 1, 1)
        std = torch.tensor([0.229, 0.224, 0.225], device="cuda").view(1, 3, 1, 1)
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "in.mp4"
            src.write_bytes(base64.b64decode(inp["video_url"].split(",", 1)[1]))
            reader = iio.get_reader(str(src))
            fps = reader.get_meta_data().get("fps", FPS)
            frames = [f for f in reader]
            reader.close()
        h, w = frames[0].shape[:2]
        part = out_base.with_suffix(".part.mp4")
        writer = iio.get_writer(str(part), fps=fps, codec="libx264", quality=None,
                                ffmpeg_params=["-crf", "12", "-pix_fmt", "yuv420p"], macro_block_size=1)
        with torch.no_grad():
            for i in range(0, len(frames), 8):
                x = torch.from_numpy(np.stack(frames[i:i + 8])).to("cuda").permute(0, 3, 1, 2).float() / 255
                x = F.interpolate(x, size=(1024, 1024), mode="bilinear", align_corners=False)
                x = ((x - mean) / std).half()
                m = model(x)[-1].sigmoid().float()
                m = F.interpolate(m, size=(h, w), mode="bilinear", align_corners=False)
                for g in (m[:, 0].clamp(0, 1) * 255).byte().cpu().numpy():
                    writer.append_data(np.repeat(g[:, :, None], 3, axis=2))
        writer.close()
        final = out_base.with_suffix(".mp4")
        part.replace(final)
        return final, {"frames": len(frames), "width": w, "height": h}


class RifeInterpolate:
    """RIFE 4.26 (hzwer, MIT): new frames between each pair of frames, so a 24 fps clip comes back at 48 fps with
    the same length. The network code and weights come from the author's release (Practical-RIFE commit bbfd2ea
    and hzwer/RIFE RIFEv4.26_0921.zip, both MIT), unpacked under /opt/rife at build time."""

    root = Path(os.environ.get("RIFE_DIR", "/opt/rife"))

    def model(self):
        def load():
            import sys

            import torch

            sys.path.insert(0, str(self.root))
            from train_log.IFNet_HDv3 import IFNet

            net = IFNet()
            state = torch.load(self.root / "train_log" / "flownet.pkl", map_location="cpu")
            state = {k.removeprefix("module."): v for k, v in state.items()}
            missing, _ = net.load_state_dict(state, strict=False)
            if missing:
                raise RuntimeError(f"rife weights miss {len(missing)} tensors, first {missing[0]}")
            # float32 weights, fp16 math under autocast: the warp grid is float32, so a .half() net fails there.
            return net.to("cuda").eval()

        return resident("rife", load)

    def warm(self):
        self.model()

    def validate(self, inp: dict) -> str | None:
        if not re.match(r"^data:video/mp4;base64,", str(inp.get("video_url", ""))):
            return "video_url must be a data:video/mp4;base64 URI"
        if int(inp.get("multi", 2)) not in (2, 4):
            return "multi must be 2 or 4 (frames out per frame in)"
        return None

    def run(self, inp: dict, out_base: Path):
        import base64
        import tempfile

        import imageio.v2 as iio
        import numpy as np
        import torch
        import torch.nn.functional as F

        net = self.model()
        multi = int(inp.get("multi", 2))
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "in.mp4"
            src.write_bytes(base64.b64decode(inp["video_url"].split(",", 1)[1]))
            reader = iio.get_reader(str(src))
            fps = float(reader.get_meta_data().get("fps", FPS))
            frames = [f for f in reader]
            reader.close()
        if len(frames) < 2:
            raise ValueError("the clip needs at least 2 frames")
        h, w = frames[0].shape[:2]
        # The flow pyramid halves the picture 5 times: pad both sides up to a multiple of 64.
        ph, pw = -(-h // 64) * 64, -(-w // 64) * 64
        steps = [k / multi for k in range(1, multi)]
        part = out_base.with_suffix(".part.mp4")
        writer = iio.get_writer(str(part), fps=fps * multi, codec="libx264", quality=None,
                                ffmpeg_params=["-crf", "14", "-pix_fmt", "yuv420p"], macro_block_size=1)

        def tensor(batch):
            x = torch.from_numpy(np.stack(batch)).to("cuda").permute(0, 3, 1, 2).float() / 255
            return F.pad(x, (0, pw - w, 0, ph - h))

        with torch.no_grad(), torch.autocast("cuda", dtype=torch.float16):
            for i in range(0, len(frames) - 1, 8):
                a = tensor(frames[i:i + 8])
                b = tensor(frames[i + 1:i + 9])
                a = a[: b.shape[0]]
                mids = []
                for t in steps:
                    ts = torch.full((a.shape[0], 1, 1, 1), t, device="cuda", dtype=a.dtype)
                    _, _, merged = net(torch.cat((a, b), 1), ts, [16, 8, 4, 2, 1])
                    mids.append((merged[-1][:, :, :h, :w].float().clamp(0, 1) * 255).round().byte().permute(0, 2, 3, 1).cpu().numpy())
                for j in range(a.shape[0]):
                    writer.append_data(frames[i + j])
                    for m in mids:
                        writer.append_data(m[j])
            # The last source frame closes the clip; repeat it so the length stays frames / fps.
            for _ in range(multi):
                writer.append_data(frames[-1])
        writer.close()
        final = out_base.with_suffix(".mp4")
        part.replace(final)
        return final, {"frames_in": len(frames), "frames_out": len(frames) * multi, "fps": fps * multi}


ENGINES = {
    "wan22-ti2v-5b/t2v": WanT2V(),
    "wan22-ti2v-5b/i2v": WanI2V(),
    "fastwan22-ti2v-5b/t2v": FastWanT2V(),
    "fastwan22-ti2v-5b/i2v": FastWanI2V(),
    "flux2-klein-4b/t2i": FluxKlein(),
    "acestep-xl-turbo/music": AceStepMusic(),
    "seedvr2-3b/upscale": SeedVR2Upscale(),
    "birefnet/matte": BiRefNetMatte(),
    "rife426/interpolate": RifeInterpolate(),
}
