# Lip sync (5 Nov 2026)

Gap: HeyGen does this as a closed service, and a dub had the wrong mouth. LatentSync 1.6 runs as a third image of `services/gen`, on one free H100 of the owner's service. Open question for a later release: MuseTalk 1.5 (MIT, MIT) is the next engine for this pod.

- `scriptvox generate --model latentsync-16-self --video v.mp4 --audio voice.wav`: the same video with the mouth moving to the voice. Free. Any language, because it is driven by the sound.
- Terms: code Apache-2.0, weights OpenRAIL++, InsightFace `buffalo_l` non-commercial research only. They stay with their owners, and the row records them. `commercial = false`: `export` writes `commercial_ok: false` in `metadata.json`; no note is printed (13 Nov 2026); the user decides.
- Service: path `/scriptvox-lipsync` on your own host. The owner's deploy files are private.
- `scriptvox edit v.mp4 --lipsync voice.wav`: the same, then any other `edit` steps on the result. Without a row it stops with exit 10.

Measured 5 Nov 2026 on one H100 of the owner's service: TEDxGeneva clip (CC BY 3.0, not in the tree), 1080x1080, Arabic Chatterbox voice, 5.56 s. 137 s including the model load on the first job. Compared by eye, frame by frame, with the source on 5 frames: the mouth takes new shapes, the same person, a little softer in the face. Not checked: sync at speed by a person. A second run with the same files comes from the job cache in 0 s.

Fault on the way: the first deploy did not start, because the GPU nodes needed a toleration the manifest did not have. Added it.

