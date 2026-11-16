# Your own GPU service (5 Nov 2026)

Most AI rows run on a service you host: `services/gen` holds the queue and the engines for Wan, FLUX.2 klein, Chatterbox, Whisper, ACE-Step, SeedVR2, RIFE, BiRefNet, Demucs, MMAudio and LatentSync. The built-in rows name an example host and do nothing until you add yours.

- Rows: put them in `generators.toml` in the config folder (`~/.config/scriptvox`). A row with the same name replaces the built-in one; a new name is added. Give `base`, `hosts` and, for a private certificate, `ca` with its file in `<config>/ca/`.
- Key: `scriptvox keys set selfhost` (the name of the self-hosted key).
- Vision: `SCRIPTVOX_VISION_URL` or the file `vision-url` in the config folder, any OpenAI-compatible base URL; key with `scriptvox keys set vision`. With none set, nothing is sent.
- `scriptvox providers` shows each row's `service` and `key_set`.

Measured: the rows of the self-hosted pods work through this file on the workstation; an empty config sees the example rows with `key_set: false`.

Names (5 Nov 2026): gateway, engine and key `selfhost`, variable `SCRIPTVOX_SELFHOST_KEY`, row names end in `-self`. They replace older private names, with no alias: nothing is released, so no stranger has the old ones. An old brief stops with "no voice matches" or an unknown row and points to `scriptvox voices` or `scriptvox providers`. An old key entry in `keys.toml` is ignored, so set the key again with `scriptvox keys set selfhost`.

Vision model name (G136): `SCRIPTVOX_VISION_MODEL`, or the file `vision-model` in the config folder, names the model for every vision call. `judge --model` wins over it.

Update 7 Nov 2026 (audit B1, docs part): `services/gen/README.md` is the public start: the three images, the weights list (source, files, licence, date read; revisions were not pinned), one `docker run` line per image, the card it ran on, and how to point a row at your host. No image was built or pushed for this.
