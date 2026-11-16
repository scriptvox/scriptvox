# Title behind the subject (L3-D cut-out, first use)

Gap: the "text behind the subject" look (CapCut, ComfyUI BiRefNet and SAM workflows) needs a mask on every frame.
ScriptVox had no cut-out. Now a title card can stand behind the subject, with a free mask row on its own GPU pod.

- Model: BiRefNet (ZhengPeng7, MIT), 1024x1024 input, fp16 on one H100, the model code shipped with the weights
  (loaded as a namespace package because `birefnet.py` imports its config relatively). Weights relayed as
  `weights-birefnet:v1` (424 MB). Engine `birefnet/matte`: video in, a grey mp4 of the same size and frame count out
  (white = subject). Image `rawivox-gen:20261103c` (adds timm 1.0.29, kornia 0.8.2).
- Row `birefnet-self` (kind `matte`, free). `scriptvox generate --model birefnet-self --video clip.mp4`.
- Brief: `card = {"kind": "title", "text": "...", "behind": true}`. Only a title may stand behind (brief check).
  The title is 3 times the caption size, centred at 36 percent of the height, outline style with a soft shadow, no box.
- `make`: the behind titles go into their own `behind.ass` (same header, no captions). For each title window the
  visual track without text is cut (`behind<n>.mp4`), sent to the matte row (`behind<n>_matte`, resumed on a rerun),
  and the graph draws `behind.ass` on the picture, then lays the picture back over it where the mask is white
  (`trim`, `alphamerge`, `setpts` to the window, `overlay`), then draws the captions on top. Captions always stay in
  front of the subject.
- Placement: one FFmpeg pass shrinks every mask frame to 1 x 192 grey pixels (across the title's measured width,
  centred; 15 to 100 percent), so
  each row has a subject share averaged over the window. The title centre moves between 15 and 65 percent of the
  height to where its worst line is hidden nearest 20 percent (the subject clearly in front, each line readable);
  a subject that crosses nowhere keeps 36 percent. `behind.ass` gets the new `\pos`.
- Gate `behind`: the placed height and the worst line's hidden share; warn above 40 percent with a fix line.
- Arabic: letters join on the baseline, so hiding the middle of a word costs more. Aim 8 percent, warn above 25.
  Measured: at the Latin aim (20) the dhow awning covered the baseline of "الخليج" (21 percent, not readable on a
  zoomed crop); at 8 it moved above the awning (9 percent, the mast and ropes in front, readable). "المدينة" over
  the towers: 26 percent, warn, borderline by eye.
- Measured 3 Nov: a 5 s Wan clip masked in 16 s round trip (8 frames in 0.28 s on the GPU); the mask kept the mast and
  ropes. A 2-scene English video: the dhow mast and ropes pass in front of "THE GULF" (2 percent hidden, readable,
  checked by eye on a zoomed crop); two towers hid 64 percent of "ABU DHABI" (warn; by eye unreadable). Make with
  both titles: 27 s on the workstation plus the pod time. The first try had the card box behind the title, which hid
  the effect; the outline style fixed it.
- Measured with placement: "THE GULF" moved from 36 to 47 percent, into the rigging, 21 percent hidden, readable.
  "ABU DHABI" wraps to two lines; the first try scored the two lines as one band (31 percent, pass) while "DHABI"
  was almost all behind the towers; scored per line it is 59 percent at best, warn. "CITY" over the same towers: 15
  percent of the height, 24 percent hidden, behind the tower tops and the crane, readable (`night-output/behind/`).
- Text width (3 Nov): the band is the widest line of the title measured with the caption font at the title size,
  not the middle 80 percent of the frame. Measured on the same masks: "CITY" over the towers moved from 15 to 17
  percent of the height and the hidden share from 24 to 20 (the aim); "THE GULF" 21 to 18; "ABU DHABI" stays a warn
  at 59 (the towers fill its width). Checked by eye (`night-output/behind/width-city.png`).
- Tests: the graph (unit), the style swap and the card check (unit); a real render checked by eye.
