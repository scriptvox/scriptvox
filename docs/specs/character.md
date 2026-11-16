# Brief `character`: the same person in every scene (5 Nov 2026)

Gap: MoneyPrinterTurbo #21 and users of every text-to-video tool: a person changes from shot to shot. ComfyUI does it with a hand-built graph.

- Brief `character`: `{"prompt": "..."}`, `{"file": "hero.png"}`, or both (the file is the picture, the words are for clip rows). Scene `generate.character: false` leaves it out.
- A prompt: one picture from an image row with `takes_reference` (`flux2-klein-4b-self`), framed full body on a plain background, kept in the content store (same prompt, aspect and `style`: the same person, also in another run). `style` applies.
- Each still and first frame sends the picture as `image_urls` to the row. A clip with no first frame gets one from its own prompt, then image to video.
- A clip row never sees the picture, so a clip prompt starts with the character's words.
- Content keys hold the character, so a new one makes new pictures, and a change of look makes none. A scene with its own `image` is not touched.
- Service: `FluxKlein` takes up to 4 data URIs (`image_urls`). Image `rawivox-gen:20261105a`. Only free rows take a reference today, so the plan shows no cost for the picture. A paid row needs that line first.

Measured before the service change, on the pod, FLUX.2 klein 768x1344: with the reference 1.9 s a picture, jacket, trousers, hair and glasses the same in a market and on a site; with text alone the jacket changed cut and the trousers changed colour.

Real run, r_1f9355 and r_03f64f (4 scenes, Edge, 9:16): stills through the row with the same hard hat, abaya and glasses by eye; the skyline with `character: false` has no person. All gates pass, 3 scenes reused free on the second run.

Fault found by eye: clip scene on FastWan, frame 0 exact, by frame 15 the abaya turned grey and the glasses became sunglasses. With the words in the clip prompt: hat, glasses and scarf hold, the navy still fades toward grey. Base Wan (r_b24a7b, about 4 min) holds the person and the navy to frame 60. So FastWan loses exact colours; base Wan, the default free row, does not.

## Two or more people (5 Nov 2026)

Brief `characters`: a list, each with a `name`. Scene `generate.who` names who is in it, at most 4 (the row's limit); default all. Each picture is made once, `character.png`, `character2.png`. A clip prompt starts with the words of those in the scene. `character` and `characters` together stop at `plan`.

Real run r_82d767, 4 scenes: a woman and a man each hold their clothes and face, alone and together on one shore, and the sea wall with `character: false` has no person. All gates pass. Small drift: her trousers went from black to brown in one still. Stills from the reference, 1.9 s each.

A picture out of the store is copied to `character.png`, `character2.png` by this run's order, whatever name it was first kept under. More than 4 in a picture, or more than 28 MB of file references in one scene, stops at `plan`. A scene's content key has no `character` field when it has no cast, so older clips keep their key.
