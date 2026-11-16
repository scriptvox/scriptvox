# Last frame: `generate --last-image`, brief `last_frame` (G074, 8 Nov 2026)

A clip that goes from a first picture to a given last picture, as hosted video services offer it (public product
behaviour and docs). Our own design.

- Row field `takes_last_frame`: the request field the service reads the last picture from (for example
  `"tail_image_url"`). `scriptvox providers` lists it. A row needs an image to video endpoint for it to count.
- None of the built-in rows sets it yet: the field name must come from the service's own docs, and they could not be
  read from this machine on 8 Nov (no network to the docs host). A row in the user's `generators.toml` overlay can set it.
  The free self-hosted rows (Wan 2.2 TI2V 5B, FastWan) do not take a last frame.
- `generate --image first.png --last-image last.png`: both pictures inside the project (png, jpg, webp), sent as data
  URIs. `--last-image` alone, or on a row without the field, is exit 2 before any plan or send.
- Brief, scene `generate.last_frame`: a file relative to the brief. Needs `image` or `first_frame`, not a still. With no
  `model`, only rows with the field are picked; none is exit 2, "no generator row takes a last frame". A named row
  without it is exit 2 with the reason.
- The picture counts in the plan id by its hash, never its bytes, as the first frame does. Price: as the row's clip;
  no row is known to charge more for a last frame.

## Measured, 8 Nov 2026

| Check | Result |
|---|---|
| Fake fal queue, `kling-2.6-pro` as built in | `--image --last-image` exit 2 "takes no last frame", 0 submits; `--last-image` alone exit 2 |
| Same, overlay row with `takes_last_frame = "tail_image_url"` | plan id differs from the first-frame-only plan; approved: 1 submit, `start_image_url` (the row's `image_field`) and `tail_image_url` both PNG data URIs |
| `plan --brief` with scene `last_frame`, built-in rows | exit 2, "no generator row takes a last frame"; without a first frame, exit 2 |
| `plan --brief`, overlay row | ok: kling-2.6-pro, 5 s, $0.35, approval needed; `providers` shows `"takes_last_frame": "tail_image_url"` |

No real clip: no free row takes a last frame, and a paid call needs the owner's approval.

## Not done

The field on built-in rows (check each service's docs first). A last frame made from a prompt, like `first_frame`.
A free route: a first-last-frame model on `services/gen` (needs the cluster).
