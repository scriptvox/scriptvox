//! Own media as the shot list (PLAN-R1 §5, CAP gap 9; MPT #62, #1696).
//! Your clips and stills are the visuals, in file-name order, cut on the
//! caption pages so shots change with the speech. Path-root check from the
//! MPT #1336 idea: media must live inside the project folder.

use std::path::{Path, PathBuf};

const VIDEO: [&str; 6] = ["mp4", "mov", "mkv", "webm", "m4v", "avi"];
const IMAGE: [&str; 5] = ["jpg", "jpeg", "png", "webp", "bmp"];

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Video,
    Image,
    /// The built-in animated gradient (a scene with no clip).
    Gradient,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub path: PathBuf,
    pub kind: Kind,
    /// Where a clip starts playing, in seconds (scene `media_from`).
    pub from: f64,
    /// Where it stops: past this the clip slows or holds, it does not run on (scene `media_to`).
    pub to: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub enum MediaError {
    OutsideRoot(PathBuf),
    Missing(PathBuf),
    Empty(PathBuf),
}

/// Natural order: "clip2" before "clip10".
fn natural_key(s: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut digits = String::new();
    let mut text = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_digit() {
            if !text.is_empty() {
                out.push((0, std::mem::take(&mut text)));
            }
            digits.push(c);
        } else {
            if !digits.is_empty() {
                out.push((digits.parse().unwrap_or(u64::MAX), String::new()));
                digits.clear();
            }
            text.push(c);
        }
    }
    if !digits.is_empty() {
        out.push((digits.parse().unwrap_or(u64::MAX), String::new()));
    }
    if !text.is_empty() {
        out.push((0, text));
    }
    out
}

/// The kind of a media file, by extension.
pub fn kind_of(p: &Path) -> Option<Kind> {
    let ext = p.extension()?.to_string_lossy().to_lowercase();
    if VIDEO.contains(&ext.as_str()) {
        Some(Kind::Video)
    } else if IMAGE.contains(&ext.as_str()) {
        Some(Kind::Image)
    } else {
        None
    }
}

/// Lists the media in `dir`, which must resolve inside `root`.
pub fn list(dir: &Path, root: &Path) -> Result<Vec<Item>, MediaError> {
    let real = dir
        .canonicalize()
        .map_err(|_| MediaError::Missing(dir.to_path_buf()))?;
    let root = root
        .canonicalize()
        .map_err(|_| MediaError::Missing(root.to_path_buf()))?;
    if !real.starts_with(&root) {
        return Err(MediaError::OutsideRoot(real));
    }
    let mut items: Vec<Item> = std::fs::read_dir(&real)
        .map_err(|_| MediaError::Missing(dir.to_path_buf()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        // A symlink that points outside the project is outside the project.
        .filter(|p| p.canonicalize().is_ok_and(|c| c.starts_with(&root)))
        .filter_map(|p| {
            let ext = p.extension()?.to_string_lossy().to_lowercase();
            let kind = if VIDEO.contains(&ext.as_str()) {
                Kind::Video
            } else if IMAGE.contains(&ext.as_str()) {
                Kind::Image
            } else {
                return None;
            };
            Some(Item {
                path: p,
                kind,
                from: 0.0,
                to: None,
            })
        })
        .collect();
    items.sort_by_key(|i| natural_key(&i.path.file_name().unwrap_or_default().to_string_lossy()));
    if items.is_empty() {
        return Err(MediaError::Empty(real));
    }
    Ok(items)
}

/// Shot boundaries from caption page starts: shot i runs from page i's start
/// (0 for the first) to the next page's start (`total` for the last).
pub fn shots(page_starts: &[f64], total: f64) -> Vec<(f64, f64)> {
    let mut cuts: Vec<f64> = page_starts.to_vec();
    if cuts.is_empty() {
        cuts.push(0.0);
    }
    cuts[0] = 0.0;
    (0..cuts.len())
        .map(|i| {
            (
                cuts[i],
                if i + 1 < cuts.len() {
                    cuts[i + 1]
                } else {
                    total
                },
            )
        })
        .filter(|(a, b)| b - a > 0.05)
        .collect()
}

/// Shortest shot made by splitting, in seconds.
pub const MIN_SHOT: f64 = 1.2;

/// Every file the user gave should appear: while there are fewer shots than
/// items, split the longest shot in half (never below `MIN_SHOT`).
pub fn spread(mut shots: Vec<(f64, f64)>, items: usize) -> Vec<(f64, f64)> {
    while shots.len() < items {
        let (i, &(a, b)) = shots
            .iter()
            .enumerate()
            .max_by(|x, y| (x.1 .1 - x.1 .0).total_cmp(&(y.1 .1 - y.1 .0)))
            .unwrap();
        if (b - a) / 2.0 < MIN_SHOT {
            break;
        }
        let mid = (a + b) / 2.0;
        shots.splice(i..=i, [(a, mid), (mid, b)]);
    }
    shots
}

/// Crossfade between shots, in seconds.
pub const XFADE: f64 = 0.25;

/// The transition names a brief may use.
pub const TRANSITIONS: &str =
    "fade, slide, cut, wipe, push, cover, dissolve, blur, circle, dip, flash, pixel, radial, wind, zoom, reveal, iris, split, slice, rise";

/// A transition as (FFmpeg xfade name, seconds). Sideways moves follow the reading direction, so an Arabic video moves
/// the other way. `cut` is a one-frame fade, so the timing stays the same as the others.
pub fn transition(name: Option<&str>, rtl: bool, fps: u32) -> Result<(&'static str, f64), String> {
    let side = |ltr, r| if rtl { r } else { ltr };
    Ok(match name.unwrap_or("fade") {
        "fade" => ("fade", XFADE),
        "slide" => (side("smoothleft", "smoothright"), 0.4),
        "cut" => ("fade", 1.0 / f64::from(fps)),
        "wipe" => (side("wipeleft", "wiperight"), 0.35),
        "push" => (side("slideleft", "slideright"), 0.35),
        "cover" => (side("coverleft", "coverright"), 0.35),
        "dissolve" => ("dissolve", 0.4),
        "blur" => ("hblur", 0.3),
        "circle" => ("circleopen", 0.45),
        "dip" => ("fadeblack", 0.5),
        "flash" => ("fadewhite", 0.3),
        "pixel" => ("pixelize", 0.4),
        "radial" => ("radial", 0.45),
        "wind" => (side("hlwind", "hrwind"), 0.4),
        // 13 Nov 2026 (docs/specs/transitions.md, "Five more names"): plain xfade kinds.
        "reveal" => (side("revealleft", "revealright"), 0.35),
        "iris" => ("circleclose", 0.45),
        "split" => ("vertopen", 0.4),
        "slice" => (side("hlslice", "hrslice"), 0.45),
        "rise" => ("slideup", 0.35),
        // Not an xfade name: see `zoom_step` and `xfade_name`.
        "zoom" => ("zoom", 0.4),
        other => return Err(format!("unknown transition {other}: use {TRANSITIONS}")),
    })
}

/// Extra seconds at the end of a shot that goes into an xfade, past the transition itself: 2 frames. FFmpeg's xfade
/// starts the next shot only at a frame of the first one at or after `offset`, and ends the video when the first one
/// stops before it. Shot lengths and offsets each round to whole frames, so with an overlap of one frame (`cut`) the
/// first shot was sometimes one frame short: the video stopped at that cut (13 Nov 2026: 97 frames of 149, and 103
/// of 206). xfade drops the frames it does not need, so the timing does not change.
pub fn xfade_tail(fps: u32) -> f64 {
    2.0 / f64::from(fps)
}

/// The xfade under a transition: `zoom` is a fade over shots that move (FFmpeg's own `zoomin` washes its middle frame out).
pub fn xfade_name(xf: &str) -> &str {
    if xf == "zoom" {
        "fade"
    } else {
        xf
    }
}

/// The filter that drives through a cut: the last `out_s` seconds of a shot push in to 145 percent, gathering speed, and
/// the first `in_s` seconds of the next settle from 120 percent to 100. One `zoompan` step on the shot, no per-pixel
/// expression (130 CPU seconds for 12 frames, 5 Nov 2026). None when the shot has no zoom on either end.
pub fn zoom_step(
    (w, h): (u32, u32),
    fps: u32,
    d: f64,
    in_s: Option<f64>,
    out_s: Option<f64>,
) -> Option<String> {
    if in_s.is_none() && out_s.is_none() {
        return None;
    }
    let frames = |s: f64| (s * f64::from(fps)).round().max(1.0);
    let mut z = "1".to_string();
    if let Some(s) = out_s {
        let (n, from) = (frames(s), frames(d) - frames(s));
        z = format!("if(gt(on,{from}),1+0.45*pow((on-{from})/{n},2),{z})");
    }
    if let Some(s) = in_s {
        let n = frames(s);
        z = format!("if(lt(on,{n}),1+0.2*pow(1-on/{n},2),{z})");
    }
    Some(format!(
        "zoompan=z='{z}':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s={w}x{h}:fps={fps},setsar=1"
    ))
}

/// A video clip shorter than its shot but at least this share of it plays slower to fill the shot.
/// Shorter clips hold their last frame: a slow-down beyond 1/0.7 (about 1.43x) looks wrong.
pub const SLOW_MIN: f64 = 0.7;

/// A clip of 48 fps or more (brief `smooth`: RIFE doubled its frames) may play up to 2 times slower: it still shows
/// 24 real frames a second.
pub const SLOW_MIN_SMOOTH: f64 = 0.5;

/// The `setpts` factor that stretches a clip of `len` seconds at `fps` to `d` seconds, or None to play it as is
/// (it is long enough) or hold its last frame (it is too short to slow down well).
pub fn stretch((len, fps): (f64, f64), d: f64) -> Option<f64> {
    let min = if fps >= 47.9 {
        SLOW_MIN_SMOOTH
    } else {
        SLOW_MIN
    };
    (len > 0.0 && len < d && len >= min * d).then(|| d / len)
}

/// A smaller picture over `label` during `span`, starting at its own first frame, with a thin white edge. A clip that
/// ends early holds its last frame. Bottom corners stay above the captions.
pub fn inset_graph(
    inset: &crate::brief::Inset,
    input: usize,
    label: &str,
    out: &str,
    (a, b): (f64, f64),
    (w, h): (u32, u32),
) -> String {
    let iw = ((w as f32 * inset.width.unwrap_or(0.4)) as u32 / 2 * 2).max(2);
    let m = w * 4 / 100;
    let e = (w / 270).max(2);
    let corner = inset.corner.as_deref().unwrap_or("top_right");
    let x = if corner.ends_with("left") {
        m.to_string()
    } else {
        format!("W-w-{m}")
    };
    let y = if corner.starts_with("top") {
        (h * 8 / 100).to_string()
    } else {
        // Above a two-line caption.
        format!("{}-h", h * if w > h { 72 } else { 66 } / 100)
    };
    format!(
        "[{input}:v]scale={iw}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2,setsar=1,pad=iw+{}:ih+{}:{e}:{e}:white,setpts=PTS-STARTPTS+{a:.3}/TB[ip{input}];{label}[ip{input}]overlay={x}:{y}:enable='between(t,{a:.3},{b:.3})':eof_action=repeat{out}",
        // A tall clip on a wide frame fits by its height.
        h * if w > h { 64 } else { 50 } / 100,
        2 * e,
        2 * e
    )
}

/// A full-frame layer with alpha over one scene: it fits the frame without stretching, starts at the scene, and goes at
/// the scene's end or its own.
pub fn overlay_graph(
    input: usize,
    label: &str,
    out: &str,
    (a, b): (f64, f64),
    (w, h): (u32, u32),
) -> String {
    format!(
        "[{input}:v]scale={w}:{h}:force_original_aspect_ratio=decrease,format=rgba,setpts=PTS-STARTPTS+{a:.3}/TB[ov{input}];{label}[ov{input}]overlay=(W-w)/2:(H-h)/2:enable='between(t,{a:.3},{b:.3})':eof_action=pass{out}"
    )
}

/// The colour look of the whole picture (brief `look`), as an FFmpeg filter on the background before captions and
/// cards, so text keeps its own colours. `natural` (default) changes nothing. No grain: per-frame noise would hide
/// a frozen picture from the slideshow gate.
pub fn look(name: Option<&str>) -> Result<Option<String>, String> {
    let named = match name.unwrap_or("natural") {
        "natural" => None,
        "warm" => Some("colortemperature=temperature=5000:mix=0.6,eq=saturation=1.08"),
        "cool" => Some("colortemperature=temperature=8500:mix=0.6"),
        "film" => Some("curves=preset=medium_contrast,eq=saturation=0.85,vignette=angle=PI/5"),
        "punchy" => Some("eq=contrast=1.08:saturation=1.25,unsharp=5:5:0.4"),
        "mono" => Some("hue=s=0,eq=contrast=1.1"),
        file if file.to_ascii_lowercase().ends_with(".cube") => {
            // Checked here: a bad LUT would stop FFmpeg only after the voice was made and paid for.
            let real = Path::new(file)
                .canonicalize()
                .map_err(|e| format!("look {file}: {e}"))?;
            let root = Path::new(".").canonicalize().map_err(|e| e.to_string())?;
            if !real.starts_with(&root) {
                return Err(format!("look {file} is outside this project folder"));
            }
            let text = std::fs::read_to_string(&real).map_err(|e| format!("look {file}: {e}"))?;
            let size = text
                .lines()
                .find_map(|l| l.trim().strip_prefix("LUT_3D_SIZE"))
                .and_then(|n| n.trim().parse::<usize>().ok());
            let rows = text
                .lines()
                .filter(|l| {
                    let parts: Vec<&str> = l.split_whitespace().collect();
                    parts.len() == 3 && parts.iter().all(|p| p.parse::<f32>().is_ok())
                })
                .count();
            if size.is_none_or(|n| rows < n * n * n) {
                return Err(format!(
                    "look {file} is not a whole 3D LUT: it needs a LUT_3D_SIZE line and that many rows cubed"
                ));
            }
            let abs = std::path::absolute(file).map_err(|e| e.to_string())?;
            return Ok(Some(format!(
                "lut3d=file={}",
                crate::filterpath::filter_path(&abs)
            )));
        }
        other => {
            return Err(format!(
                "unknown look {other}: use natural, warm, cool, film, punchy, mono or a .cube file"
            ))
        }
    };
    Ok(named.map(String::from))
}

/// Fits a picture into `w`x`h` with nothing cut: the whole picture, over a blurred copy that fills the frame. Blurred at a
/// quarter of the size: at 1080x1920 the blur costs a sixteenth, and it looks the same.
pub fn fit_blur(w: u32, h: u32, tag: usize) -> String {
    format!(
        "split[fitbg{tag}][fitfg{tag}];[fitbg{tag}]scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},scale={qw}:{qh},boxblur=10:2,scale={w}:{h},setsar=1[fitb{tag}];[fitfg{tag}]scale={w}:{h}:force_original_aspect_ratio=decrease,setsar=1[fitf{tag}];[fitb{tag}][fitf{tag}]overlay=(W-w)/2:(H-h)/2",
        qw = (w / 8).max(1) * 2,
        qh = (h / 8).max(1) * 2,
    )
}

/// A camera move on a video shot (scene `camera`), as filters on the frame already cropped to `w`x`h`, over the
/// shot's `d` seconds. Push and pull use `zoompan` on a 2x frame (half-pixel steps, as for stills); pans use a
/// per-frame `crop` on a 12 percent larger frame (`exact`: 1 px steps, the default rounds to 2 px, a visible stutter on a slow pan).
pub fn camera(name: &str, (w, h): (u32, u32), d: f64, fps: u32) -> Result<String, String> {
    let n = (d * f64::from(fps)).round().max(1.0);
    let zoom = |z: &str| {
        format!(
            "scale={}:{},zoompan=z='{z}':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s={w}x{h}:fps={fps}",
            w * 2,
            h * 2
        )
    };
    let pan = |x: &str| {
        let big = |v: u32| (f64::from(v) * 1.12 / 2.0).round() as u32 * 2;
        format!(
            "scale={}:{},crop={w}:{h}:x='{x}':y='(ih-oh)/2':exact=1",
            big(w),
            big(h)
        )
    };
    Ok(match name {
        "push_in" => zoom(&format!("1+0.12*on/{n}")),
        "pull_out" => zoom(&format!("1.12-0.12*on/{n}")),
        "pan_right" => pan(&format!("(iw-ow)*min(t/{d:.3},1)")),
        "pan_left" => pan(&format!("(iw-ow)*(1-min(t/{d:.3},1))")),
        other => {
            return Err(format!(
                "unknown camera {other}: use push_in, pull_out, pan_left or pan_right"
            ))
        }
    })
}

/// FFmpeg inputs and the filter chain for the visual track. Items cycle when
/// there are more shots than items. `lens` holds each item's length in seconds and frame rate when known (empty
/// is fine): a video a little shorter than its shot slows down, see [`stretch`]. `moves` holds a camera move per
/// shot (empty is fine), see [`camera`]; stills keep their own push-in. `into` holds a transition into each shot where a
/// scene asks for its own (empty is fine), the others use `tr`. Output label: `[bg]`.
#[allow(clippy::too_many_arguments)]
pub fn graph(
    items: &[Item],
    lens: &[Option<(f64, f64)>],
    moves: &[Option<String>],
    shots: &[(f64, f64)],
    frame: (u32, u32),
    fps: u32,
    tr: (&str, f64),
    into: &[Option<(&str, f64)>],
) -> (Vec<String>, String) {
    graph_part(items, lens, moves, shots, frame, fps, tr, into, None)
}

/// Some shots of the visual track for a render in more than one process (docs/specs/render-split.md).
pub struct Part<'a> {
    /// The first and the last shot of the part, as places in the full shot list.
    pub shots: (usize, usize),
    /// The frame of the full track where shot `shots.0` starts, see [`shot_frame`].
    pub origin: u64,
    /// The arguments of a `format` filter with the pixel format, colour matrix and range of the full track
    /// (`split::Track::pin`): every shot ends in them, so the xfades work on the same numbers.
    pub format: &'a str,
}

/// The frame of the visual track where a shot starts: FFmpeg's xfade rounds the `offset` that [`graph`] prints (3
/// decimals) to whole frames, halves up.
pub fn shot_frame(shots: &[(f64, f64)], i: usize, fps: u32) -> u64 {
    frames_of(shots[i].0 - shots[0].0, fps)
}

/// Seconds as FFmpeg's xfade counts them in frames: the 3 decimals that the graph prints, rounded, halves up.
pub fn frames_of(secs: f64, fps: u32) -> u64 {
    let ms: u64 = format!("{secs:.3}").replace('.', "").parse().unwrap_or(0);
    (ms * u64::from(fps) + 500) / 1000
}

/// [`graph`], or with `part` only the shots of that part: each shot is built as in the full graph (its length, its
/// motion and its file follow its place in the full list), the FFmpeg inputs count from 0, and the xfade offsets
/// are whole frames from the part's origin. The output `[bg]` is then the full track from frame `origin`.
#[allow(clippy::too_many_arguments)]
pub fn graph_part(
    items: &[Item],
    lens: &[Option<(f64, f64)>],
    moves: &[Option<String>],
    shots: &[(f64, f64)],
    (w, h): (u32, u32),
    fps: u32,
    tr: (&str, f64),
    into: &[Option<(&str, f64)>],
    part: Option<&Part>,
) -> (Vec<String>, String) {
    let tr_into = |i: usize| into.get(i).copied().flatten().unwrap_or(tr);
    let mut inputs = Vec::new();
    let mut chains = Vec::new();
    let n = shots.len();
    if n == 0 {
        return (inputs, String::new());
    }
    let (first, last) = part.map_or((0, n - 1), |p| p.shots);
    for (i, (a, b)) in shots.iter().enumerate().take(last + 1).skip(first) {
        // The FFmpeg input of this shot.
        let inp = i - first;
        let item = &items[i % items.len()];
        let len = lens.get(i % items.len()).copied().flatten();
        // Every shot but the last runs longer by the transition, to overlap the next one. `d` times the motion;
        // `dt` is the length of the stream, with the tail the xfade needs.
        let d = b - a + if i + 1 < n { tr_into(i + 1).1 } else { 0.0 };
        let dt = d + if i + 1 < n { xfade_tail(fps) } else { 0.0 };
        let mv = moves
            .get(i)
            .and_then(|m| m.as_deref())
            .and_then(|m| camera(m, (w, h), d, fps).ok())
            .map(|f| format!("{f},"))
            .unwrap_or_default();
        let p = item.path.to_string_lossy().into_owned();
        match item.kind {
            Kind::Video => {
                if item.from > 0.0 {
                    inputs.extend(["-ss".into(), format!("{:.3}", item.from)]);
                }
                // The part ends here: reading on would show whatever comes next in the clip.
                let read = item.to.map_or(dt, |t| dt.min(t - item.from));
                inputs.extend(["-t".into(), format!("{read:.3}"), "-i".into(), p]);
                // Fill the frame (centre crop). `framerate` blends to the output rate, so a 24 or 25 fps clip moves
                // evenly (`fps` repeats every 4th or 5th frame: a visible judder). A clip a little short plays slower, a very short one
                // holds its last frame (tpad stays as the guard for rounding). The camera comes after tpad, so it moves over the whole shot.
                let slow = len
                    .and_then(|l| stretch(l, d))
                    .map(|f| format!("setpts={f:.4}*PTS,"))
                    .unwrap_or_default();
                chains.push(format!(
                    "[{inp}:v]{slow}scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1,framerate=fps={fps},fps={fps},setsar=1,tpad=stop_mode=clone:stop_duration={dt:.3},{mv}setsar=1,trim=duration={dt:.3},setpts=PTS-STARTPTS[s{i}]"
                ));
            }
            Kind::Gradient => {
                inputs.extend([
                    "-f".into(),
                    "lavfi".into(),
                    "-i".into(),
                    crate::template::gradient(w, h, fps, dt),
                ]);
                chains.push(format!(
                    "[{inp}:v]setsar=1,fps={fps},trim=duration={dt:.3},setpts=PTS-STARTPTS[s{i}]"
                ));
            }
            Kind::Image => {
                inputs.extend([
                    "-loop".into(),
                    "1".into(),
                    "-framerate".into(),
                    fps.to_string(),
                    "-t".into(),
                    format!("{dt:.3}"),
                    "-i".into(),
                    p,
                ]);
                // Ken Burns: a slow push-in from 100% to about 110%, rendered at 2x for smooth motion.
                let frames = (d * f64::from(fps)).round().max(1.0);
                // A scene `camera` replaces the default push-in.
                if !mv.is_empty() {
                    chains.push(format!(
                        "[{inp}:v]scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1,fps={fps},{mv}setsar=1,trim=duration={dt:.3},setpts=PTS-STARTPTS[s{i}]"
                    ));
                    continue;
                }
                chains.push(format!(
                    "[{inp}:v]scale={}:{}:force_original_aspect_ratio=increase,crop={}:{},zoompan=z='1+0.1*on/{frames}':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d=1:s={w}x{h}:fps={fps},setsar=1,trim=duration={dt:.3},setpts=PTS-STARTPTS[s{i}]",
                    w * 2, h * 2, w * 2, h * 2
                ));
            }
        }
    }
    for (i, (a, b)) in shots.iter().enumerate().take(last + 1).skip(first) {
        let next = (i + 1 < n)
            .then(|| tr_into(i + 1))
            .filter(|t| t.0 == "zoom");
        let d = b - a + if i + 1 < n { tr_into(i + 1).1 } else { 0.0 };
        let came = (i > 0).then(|| tr_into(i)).filter(|t| t.0 == "zoom");
        if let Some(z) = zoom_step((w, h), fps, d, came.map(|t| t.1), next.map(|t| t.1)) {
            let end = format!("[s{i}]");
            let c = &mut chains[i - first];
            let body = c.strip_suffix(&end).unwrap_or(c).to_string();
            *c = format!("{body},{z}{end}");
        }
    }
    if let Some(p) = part {
        for (c, i) in chains.iter_mut().zip(first..) {
            let end = format!("[s{i}]");
            let body = c.strip_suffix(&end).unwrap_or(c).to_string();
            *c = format!("{body},format={}{end}", p.format);
        }
    }
    // Crossfades: shot i+1 starts where shot i ends in the timeline, so the total
    // length (and the audio and captions) stays exactly the same.
    if first == last {
        chains.push(format!("[s{first}]null[bg]"));
    } else {
        let mut prev = format!("s{first}");
        for i in first + 1..=last {
            let out = if i == last {
                "bg".to_string()
            } else {
                format!("x{i}")
            };
            let (xf, xd) = tr_into(i);
            let xf = xfade_name(xf);
            let offset = match part {
                None => format!("{:.3}", shots[i].0 - shots[0].0),
                // Whole frames, in microseconds: FFmpeg rounds them back to the same frames.
                Some(p) => format!(
                    "{:.6}",
                    (shot_frame(shots, i, fps) - p.origin) as f64 / f64::from(fps)
                ),
            };
            chains.push(format!(
                "[{prev}][s{i}]xfade=transition={xf}:duration={xd:.3}:offset={offset}[{out}]"
            ));
            prev = out;
        }
    }
    (inputs, chains.join(";"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inset_shows_in_its_scene_above_the_captions() {
        let inset = crate::brief::Inset {
            file: "a.mp4".into(),
            corner: Some("bottom_left".into()),
            width: None,
        };
        let g = inset_graph(&inset, 3, "[bg]", "[in1]", (2.0, 5.5), (1080, 1920));
        assert!(g.starts_with("[3:v]scale=432:960:force_original"), "{g}");
        assert!(g.contains("setpts=PTS-STARTPTS+2.000/TB"), "{g}");
        assert!(
            g.contains("[bg][ip3]overlay=43:1267-h:enable='between(t,2.000,5.500)'"),
            "{g}"
        );
    }

    #[test]
    fn zoom_is_a_fade_over_shots_that_move() {
        let item = Item {
            path: "a.mp4".into(),
            kind: Kind::Gradient,
            from: 0.0,
            to: None,
        };
        let t = transition(Some("zoom"), false, 30).unwrap();
        let (_, g) = graph(
            &[item],
            &[],
            &[],
            &[(0.0, 2.0), (2.0, 4.0), (4.0, 6.0)],
            (1080, 1920),
            30,
            t,
            &[],
        );
        assert!(
            g.contains("xfade=transition=fade:duration=0.400:offset=2.000"),
            "{g}"
        );
        assert!(!g.contains("transition=zoom"), "{g}");
        let shot = |i: usize| g.split(';').nth(i).unwrap().to_string();
        // The first shot only pushes in, the last only settles, the middle does both, each ends on its own label.
        assert!(
            shot(0).contains("if(gt(on,60),1+0.45*pow((on-60)/12,2),1)")
                && shot(0).ends_with("[s0]"),
            "{g}"
        );
        assert!(
            shot(2).contains("if(lt(on,12),1+0.2*pow(1-on/12,2),1)") && shot(2).ends_with("[s2]"),
            "{g}"
        );
        assert!(
            shot(1).contains("if(lt(on,12)") && shot(1).contains("if(gt(on,60)"),
            "{g}"
        );
        // Another transition adds no step.
        assert_eq!(zoom_step((1080, 1920), 30, 2.0, None, None), None);
    }

    #[test]
    fn every_named_transition_exists_and_a_scene_can_pick_its_own() {
        for name in TRANSITIONS.split(", ") {
            assert!(transition(Some(name), false, 30).is_ok(), "{name}");
        }
        assert_eq!(transition(Some("wipe"), true, 30).unwrap().0, "wiperight");
        // The five names of 13 Nov 2026: the sideways ones turn in Arabic, the others do not.
        for (name, ltr, rtl) in [
            ("reveal", "revealleft", "revealright"),
            ("slice", "hlslice", "hrslice"),
            ("iris", "circleclose", "circleclose"),
            ("split", "vertopen", "vertopen"),
            ("rise", "slideup", "slideup"),
        ] {
            assert_eq!(transition(Some(name), false, 30).unwrap().0, ltr);
            assert_eq!(transition(Some(name), true, 30).unwrap().0, rtl);
        }
        let item = Item {
            path: "a.mp4".into(),
            kind: Kind::Gradient,
            from: 0.0,
            to: None,
        };
        let (_, g) = graph(
            &[item],
            &[],
            &[],
            &[(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
            (108, 192),
            30,
            ("fade", XFADE),
            &[None, None, Some(("radial", 0.45))],
        );
        assert!(
            g.contains("xfade=transition=fade:duration=0.250:offset=1.000"),
            "{g}"
        );
        assert!(
            g.contains("xfade=transition=radial:duration=0.450:offset=2.000"),
            "{g}"
        );
    }

    /// The scene times of the brief that lost its last scene on 13 Nov 2026 (97 frames of 149 at 24 fps).
    #[test]
    fn a_shot_runs_two_frames_past_its_cut_so_the_xfade_reaches_the_next_shot() {
        let item = Item {
            path: "a.png".into(),
            kind: Kind::Image,
            from: 0.0,
            to: None,
        };
        let cuts = [(0.0, 1.317), (1.317, 2.645), (2.645, 4.028), (4.028, 6.21)];
        let tr = transition(Some("cut"), false, 24).unwrap();
        let (inputs, g) = graph(&[item], &[], &[], &cuts, (108, 192), 24, tr, &[]);
        // Shot 3 is 1.383 s plus the one frame of the cut plus the tail: 36 frames. With 34 the video stopped here.
        assert!(
            g.contains("trim=duration=1.508,setpts=PTS-STARTPTS[s2]"),
            "{g}"
        );
        assert!(inputs.contains(&"1.508".to_string()), "{inputs:?}");
        for (i, (a, b)) in cuts.iter().enumerate().take(3) {
            let frames = |s: f64| (s * 24.0).round();
            // Each shot holds a frame at the next offset, however the two round.
            assert!(
                frames(b - a + tr.1 + xfade_tail(24)) - 1.0 >= frames(*b) - frames(*a) + 1.0,
                "{i}"
            );
        }
        // The last shot has no tail, and the motion keeps its own length.
        assert!(
            g.contains("trim=duration=2.182,setpts=PTS-STARTPTS[s3]"),
            "{g}"
        );
        assert!(g.contains("on/33'"), "{g}");
    }

    #[test]
    fn transitions_follow_the_reading_direction() {
        assert_eq!(transition(None, false, 30).unwrap(), ("fade", XFADE));
        assert_eq!(
            transition(Some("slide"), false, 30).unwrap().0,
            "smoothleft"
        );
        assert_eq!(
            transition(Some("slide"), true, 30).unwrap().0,
            "smoothright"
        );
        assert!((transition(Some("cut"), false, 30).unwrap().1 - 1.0 / 30.0).abs() < 1e-9);
        assert!(
            transition(Some("zoomin"), false, 30).is_err(),
            "FFmpeg zoomin washes the middle frame out"
        );
        let items = [
            Item {
                path: "a.mp4".into(),
                kind: Kind::Video,
                from: 0.0,
                to: None,
            },
            Item {
                path: "b.mp4".into(),
                kind: Kind::Video,
                from: 0.0,
                to: None,
            },
        ];
        let (_, g) = graph(
            &items,
            &[],
            &[],
            &[(0.0, 1.0), (1.0, 2.0)],
            (1080, 1920),
            30,
            ("smoothleft", 0.4),
            &[],
        );
        assert!(
            g.contains("xfade=transition=smoothleft:duration=0.400:offset=1.000[bg]"),
            "{g}"
        );
        assert!(g.contains("-t") || g.contains("trim=duration=1.467"), "{g}");
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["clip10.mp4", "clip2.mp4", "Clip1.mp4"];
        v.sort_by_key(|s| natural_key(s));
        assert_eq!(v, ["Clip1.mp4", "clip2.mp4", "clip10.mp4"]);
    }

    #[test]
    fn shots_follow_pages_and_cover_the_whole_video() {
        assert_eq!(
            shots(&[0.1, 3.2, 6.0], 8.5),
            vec![(0.0, 3.2), (3.2, 6.0), (6.0, 8.5)]
        );
        assert_eq!(shots(&[], 4.0), vec![(0.0, 4.0)]);
    }

    #[test]
    fn spread_uses_every_item_but_keeps_shots_long_enough() {
        let s = spread(vec![(0.0, 6.0), (6.0, 8.0)], 4);
        assert_eq!(s.len(), 4);
        assert!(s.iter().all(|(a, b)| b - a >= MIN_SHOT));
        assert_eq!((s[0].0, s[3].1), (0.0, 8.0));
        assert!(s.windows(2).all(|w| w[0].1 == w[1].0), "no gaps: {s:?}");
        assert_eq!(spread(vec![(0.0, 2.0)], 5), vec![(0.0, 2.0)]);
    }

    #[test]
    fn outside_root_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("a.mp4"), b"x").unwrap();
        assert!(matches!(
            list(outside.path(), root.path()),
            Err(MediaError::OutsideRoot(_))
        ));
    }

    #[test]
    fn a_camera_move_runs_over_a_held_last_frame() {
        let item = Item {
            path: "a.mp4".into(),
            kind: Kind::Video,
            from: 0.0,
            to: None,
        };
        let (_, g) = graph(
            &[item],
            &[Some((5.0, 24.0))],
            &[Some("pan_left".into())],
            &[(0.0, 10.0)],
            (1080, 1920),
            30,
            ("fade", 0.0),
            &[],
        );
        assert!(
            g.find("tpad").unwrap() < g.find("crop=1080:1920:x=").unwrap(),
            "{g}"
        );
    }

    #[test]
    fn camera_moves_are_known_or_refused() {
        assert!(camera("push_in", (1080, 1920), 5.0, 30)
            .unwrap()
            .contains("z='1+0.12*on/150'"));
        assert!(camera("pan_left", (1080, 1920), 5.0, 30)
            .unwrap()
            .contains("scale=1210:2150,crop=1080:1920"));
        assert!(camera("orbit", (1080, 1920), 5.0, 30)
            .unwrap_err()
            .contains("use push_in"));
        let items = vec![Item {
            path: "a.mp4".into(),
            kind: Kind::Video,
            from: 0.0,
            to: None,
        }];
        let (_, g) = graph(
            &items,
            &[],
            &[Some("pull_out".into())],
            &[(0.0, 5.0)],
            (1080, 1920),
            30,
            ("fade", XFADE),
            &[],
        );
        assert!(g.contains("zoompan=z='1.12-0.12*on/150'"), "{g}");
    }

    #[test]
    fn looks_are_known_or_refused() {
        assert_eq!(look(None).unwrap(), None);
        assert_eq!(look(Some("natural")).unwrap(), None);
        assert!(look(Some("film")).unwrap().unwrap().contains("vignette"));
        assert!(look(Some("sepia")).unwrap_err().contains("use natural"));
        // Test files live in the project: a file outside is refused.
        let d = tempfile::tempdir_in(".").unwrap();
        let bad = d.path().join("bad.cube");
        std::fs::write(&bad, "hello").unwrap();
        assert!(look(bad.to_str()).unwrap_err().contains("LUT_3D_SIZE"));
        let cut = d.path().join("cut.cube");
        std::fs::write(&cut, "LUT_3D_SIZE 2\n0 0 0\n").unwrap();
        assert!(look(cut.to_str()).unwrap_err().contains("LUT_3D_SIZE"));
        let good = d.path().join("good.cube");
        std::fs::write(&good, format!("LUT_3D_SIZE 2\n{}", "0 0 0\n".repeat(8))).unwrap();
        assert!(look(good.to_str()).unwrap().unwrap().starts_with("lut3d="));
        assert!(look(Some("/no/such.cube")).is_err());
        let outside = tempfile::tempdir().unwrap();
        let far = outside.path().join("far.cube");
        std::fs::write(&far, format!("LUT_3D_SIZE 2\n{}", "0 0 0\n".repeat(8))).unwrap();
        assert!(look(far.to_str()).unwrap_err().contains("outside"));
    }

    #[test]
    fn short_clip_slows_or_holds() {
        assert_eq!(stretch((4.0, 24.0), 5.0), Some(1.25));
        assert_eq!(stretch((3.0, 24.0), 5.0), None); // 60 percent: hold the last frame
        assert_eq!(stretch((3.0, 48.0), 5.0), Some(5.0 / 3.0)); // smoothed: 2x slower is fine
        assert_eq!(stretch((2.0, 48.0), 5.0), None);
        assert_eq!(stretch((6.0, 24.0), 5.0), None); // long enough
        assert_eq!(stretch((0.0, 24.0), 5.0), None);
        let items = vec![Item {
            path: "a.mp4".into(),
            kind: Kind::Video,
            from: 0.0,
            to: None,
        }];
        let (_, g) = graph(
            &items,
            &[Some((4.0, 24.0))],
            &[],
            &[(0.0, 5.0)],
            (1080, 1920),
            30,
            ("fade", XFADE),
            &[],
        );
        assert!(g.starts_with("[0:v]setpts=1.2500*PTS,scale="), "{g}");
        let (_, g) = graph(
            &items,
            &[Some((2.0, 24.0))],
            &[],
            &[(0.0, 5.0)],
            (1080, 1920),
            30,
            ("fade", XFADE),
            &[],
        );
        assert!(g.starts_with("[0:v]scale="), "{g}");
    }

    #[test]
    fn items_cycle_when_shots_outnumber_them() {
        let items = vec![Item {
            path: "a.png".into(),
            kind: Kind::Image,
            from: 0.0,
            to: None,
        }];
        let (inputs, g) = graph(
            &items,
            &[],
            &[],
            &[(0.0, 1.0), (1.0, 2.0)],
            (1080, 1920),
            30,
            ("fade", XFADE),
            &[],
        );
        assert_eq!(inputs.iter().filter(|s| *s == "a.png").count(), 2);
        assert!(
            g.ends_with("[s0][s1]xfade=transition=fade:duration=0.250:offset=1.000[bg]"),
            "{g}"
        );
    }
}
