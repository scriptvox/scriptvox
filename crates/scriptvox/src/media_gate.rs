//! One check for every media file that FFmpeg or ffprobe opens (`docs/specs/media-inputs.md`).
//! FFmpeg guesses a format from the content. A file named `x.png` or `x.mp4` that is a list or a script (ffconcat,
//! HLS, DASH, SVG) makes it open other local paths or URLs. So ScriptVox reads the first bytes itself, lets only the
//! small set of plain formats below pass, and then tells FFmpeg which one it is and that only the file protocol
//! is allowed.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// One picture.
    Picture,
    /// A container that can hold a picture stream (and sound). An `.m4a` or `.mka` is this kind with sound only.
    Clip,
    /// A sound-only format.
    Sound,
}

/// What a file is. `format` is the decoder for a picture and the demuxer for the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plain {
    pub kind: Kind,
    pub format: &'static str,
}

/// The whole list, for messages.
pub const ACCEPTED: &str = "Accepted: a picture (PNG, JPEG, WebP, BMP), a video (MP4, MOV, Matroska, WebM, AVI, MPEG-TS) or a sound (WAV, MP3, FLAC, OGG, Opus, M4A, AAC).";

/// What the first bytes are. `Err` names what else it is.
pub fn sniff(head: &[u8]) -> Result<Plain, &'static str> {
    let at = |i: usize, m: &[u8]| head.get(i..i + m.len()) == Some(m);
    let ok = |kind, format| Ok(Plain { kind, format });
    let text = String::from_utf8_lossy(&head[..head.len().min(64)]).to_ascii_lowercase();
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if at(0, b"\x89PNG\r\n\x1a\n") {
        ok(Kind::Picture, "png")
    } else if at(0, &[0xff, 0xd8, 0xff]) {
        ok(Kind::Picture, "mjpeg")
    } else if at(0, b"RIFF") && at(8, b"WEBP") {
        ok(Kind::Picture, "webp")
    } else if at(0, b"BM") && head.len() > 26 && head[6..10] == [0; 4] {
        ok(Kind::Picture, "bmp")
    } else if [&b"ftyp"[..], b"moov", b"mdat", b"free", b"skip", b"wide"]
        .iter()
        .any(|m| at(4, m))
    {
        ok(Kind::Clip, "mov")
    } else if at(0, &[0x1a, 0x45, 0xdf, 0xa3]) {
        ok(Kind::Clip, "matroska")
    } else if at(0, b"RIFF") && at(8, b"AVI ") {
        ok(Kind::Clip, "avi")
    } else if at(0, &[0x47]) && at(188, &[0x47]) && at(376, &[0x47]) {
        ok(Kind::Clip, "mpegts")
    } else if at(0, b"RIFF") && at(8, b"WAVE") {
        ok(Kind::Sound, "wav")
    } else if at(0, b"fLaC") {
        ok(Kind::Sound, "flac")
    } else if at(0, b"OggS") {
        ok(Kind::Sound, "ogg")
    } else if at(0, b"ID3") {
        ok(Kind::Sound, "mp3")
    } else if head.len() > 3 && head[0] == 0xff && head[1] & 0xf6 == 0xf0 {
        // The 12 sync bits and layer 0: AAC in ADTS frames.
        ok(Kind::Sound, "aac")
    } else if head.len() > 3
        && head[0] == 0xff
        && head[1] & 0xe0 == 0xe0
        && head[1] & 0x06 != 0
        && head[2] & 0xf0 != 0xf0
    {
        // An MPEG audio frame: the sync bits, a real layer, a real bit rate.
        ok(Kind::Sound, "mp3")
    } else if text.starts_with("ffconcat") {
        Err("an ffconcat list")
    } else if text.starts_with("#extm3u") {
        Err("an HLS playlist")
    } else if text.starts_with("<?xml") || text.starts_with("<mpd") || text.starts_with("<svg") {
        Err("an XML file (a DASH list, or an SVG)")
    } else {
        Err("no picture, video or sound format ScriptVox takes")
    }
}

/// The check on a file by its first bytes. The message names the file, what it is, and what is accepted.
pub fn check(file: &Path) -> Result<Plain, String> {
    use std::io::Read;
    let refuse = |what: &str| {
        format!(
            "{} is not a media file ScriptVox opens: it is {what}. {ACCEPTED}",
            file.display()
        )
    };
    if !file.is_file() {
        return Err(refuse("not a file"));
    }
    let mut head = Vec::new();
    std::fs::File::open(file)
        .and_then(|f| f.take(4096).read_to_end(&mut head))
        .map_err(|e| format!("{}: {e}", file.display()))?;
    sniff(&head).map_err(refuse)
}

impl Plain {
    /// The arguments that go right before `-i`: the file protocol only, and the demuxer named, so FFmpeg guesses
    /// nothing from the content or the name. A picture is one file (no number pattern) with its decoder by the bytes.
    pub fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = ["-protocol_whitelist", "file", "-f"]
            .map(String::from)
            .to_vec();
        if self.kind == Kind::Picture {
            a.extend(["image2", "-pattern_type", "none", "-c:v", self.format].map(String::from));
        } else {
            a.push(self.format.into());
        }
        a
    }
}

/// For the start of a command: the first of `files` that exists and is not plain media, as the command's refusal
/// (exit 2), before FFmpeg, ffprobe or any service is called. A file that does not exist is the command's own message.
pub fn refuse<P: AsRef<Path>>(
    command: &str,
    files: impl IntoIterator<Item = P>,
) -> Option<crate::envelope::Envelope> {
    let why = files
        .into_iter()
        .filter(|p| p.as_ref().is_file())
        .find_map(|p| check(p.as_ref()).err())?;
    Some(crate::envelope::Envelope::fail(
        command,
        crate::exit::Exit::Usage,
        why,
        Some("Give a real picture, video or sound file. A list of other files (a playlist) is never opened.".into()),
    ))
}

/// A file that a service gave (a row's result, a stock clip, a free picture), right after the download. A file that
/// is not plain media is removed, and `Err` names the service and what the file is.
pub fn downloaded(from: &str, file: &Path) -> Result<Plain, String> {
    check(file).map_err(|why| {
        let _ = std::fs::remove_file(file);
        let what = why.split_once(": it is ").map_or(why.as_str(), |w| w.1);
        format!("{from} answered with a file that is not plain media: it is {what}")
    })
}

/// A checked file as an FFmpeg or ffprobe input: the gate arguments, `-i`, and the path.
pub fn input(file: &Path) -> Result<Vec<String>, String> {
    input_in(Path::new(""), file)
}

/// The same for an FFmpeg that runs in `cwd`: the path is read from there and passed as it is.
pub fn input_in(cwd: &Path, file: &Path) -> Result<Vec<String>, String> {
    let mut a = check(&cwd.join(file))?.args();
    a.extend(["-i".into(), file.display().to_string()]);
    Ok(a)
}

/// The last line of defence, in `Ffmpeg::run`: every `-i` of an argument list whose format FFmpeg would guess.
/// An input with its own `-f` (lavfi, a concat list or raw samples that ScriptVox wrote) stays as it is. Any other input must
/// be a plain media file, and gets the gate arguments. `cwd` is where FFmpeg runs: a relative path is read from there.
pub fn guard(cwd: &Path, args: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(args.len() + 8);
    // Where the options of the next input start.
    let mut from = 0;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-i" && i + 1 < args.len() {
            let opts = &args[from..i];
            if !opts.iter().any(|o| o == "-f") {
                let plain = check(&cwd.join(&args[i + 1]))?;
                let mut gate = plain.args();
                if plain.kind == Kind::Picture && opts.iter().any(|o| o == "-c:v") {
                    // The caller chose the decoder: drop the gate's own `-c:v <decoder>`, its last two words.
                    gate.truncate(gate.len() - 2);
                }
                out.extend(gate);
            }
            out.extend([args[i].clone(), args[i + 1].clone()]);
            i += 2;
            from = i;
        } else {
            out.push(args[i].clone());
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(head: &[u8]) -> Option<(Kind, &'static str)> {
        sniff(head).ok().map(|p| (p.kind, p.format))
    }

    #[test]
    fn plain_formats_pass_by_their_first_bytes() {
        assert_eq!(kind(b"\x89PNG\r\n\x1a\n...."), Some((Kind::Picture, "png")));
        assert_eq!(
            kind(&[0xff, 0xd8, 0xff, 0xe0]),
            Some((Kind::Picture, "mjpeg"))
        );
        assert_eq!(kind(b"RIFF\0\0\0\0WEBPVP8 "), Some((Kind::Picture, "webp")));
        assert_eq!(kind(b"\0\0\0\x20ftypisom"), Some((Kind::Clip, "mov")));
        assert_eq!(
            kind(&[0x1a, 0x45, 0xdf, 0xa3, 0, 0]),
            Some((Kind::Clip, "matroska"))
        );
        assert_eq!(kind(b"RIFF\0\0\0\0AVI LIST"), Some((Kind::Clip, "avi")));
        assert_eq!(kind(b"RIFF\0\0\0\0WAVEfmt "), Some((Kind::Sound, "wav")));
        assert_eq!(kind(b"fLaC\0\0\0\x22"), Some((Kind::Sound, "flac")));
        assert_eq!(kind(b"OggS\0\x02\0\0"), Some((Kind::Sound, "ogg")));
        assert_eq!(kind(b"ID3\x04\0\0\0\0"), Some((Kind::Sound, "mp3")));
        // MPEG-1 layer 3 with no ID3 tag, then ADTS (layer bits 0).
        assert_eq!(kind(&[0xff, 0xfb, 0x90, 0x64]), Some((Kind::Sound, "mp3")));
        assert_eq!(kind(&[0xff, 0xf1, 0x50, 0x80]), Some((Kind::Sound, "aac")));
    }

    #[test]
    fn lists_and_scripts_are_named_and_refused() {
        assert_eq!(
            sniff(b"ffconcat version 1.0\nfile /etc/passwd\n"),
            Err("an ffconcat list")
        );
        assert_eq!(
            sniff(b"\xef\xbb\xbf  #EXTM3U\n#EXTINF:1,\n/etc/passwd\n"),
            Err("an HLS playlist")
        );
        assert!(sniff(b"<?xml version=\"1.0\"?><MPD>").is_err());
        assert!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\">").is_err());
        assert!(sniff(b"GIF89a").is_err());
        assert!(sniff(b"file 'a.mp4'\n").is_err());
        assert!(sniff(b"").is_err());
    }

    #[test]
    fn guard_gates_a_guessed_input_and_leaves_a_named_one() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.mp4"), b"\0\0\0\x20ftypisom....").unwrap();
        std::fs::write(d.path().join("p.jpg"), b"\x89PNG\r\n\x1a\n....").unwrap();
        std::fs::write(d.path().join("list.mp4"), b"ffconcat version 1.0\n").unwrap();
        let s = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let got = guard(
            d.path(),
            &s(&[
                "-ss",
                "1",
                "-i",
                "a.mp4",
                "-f",
                "lavfi",
                "-i",
                "color=red",
                "-loop",
                "1",
                "-i",
                "p.jpg",
                "-f",
                "null",
                "-",
            ]),
        )
        .unwrap()
        .join(" ");
        assert_eq!(
            got,
            "-ss 1 -protocol_whitelist file -f mov -i a.mp4 -f lavfi -i color=red -loop 1 -protocol_whitelist file -f image2 -pattern_type none -c:v png -i p.jpg -f null -"
        );
        // The caller's own decoder stays the only one.
        let got = guard(d.path(), &s(&["-c:v", "png", "-i", "p.jpg"])).unwrap();
        assert_eq!(got.iter().filter(|g| *g == "-c:v").count(), 1);
        let e = guard(d.path(), &s(&["-i", "list.mp4"])).unwrap_err();
        assert!(
            e.contains("list.mp4") && e.contains("an ffconcat list") && e.contains("Accepted:"),
            "{e}"
        );
        assert!(guard(d.path(), &s(&["-i", "https://example.com/x.mp4"])).is_err());
        assert!(guard(d.path(), &s(&["-i", "concat:a.mp4|b.mp4"])).is_err());
    }
}
