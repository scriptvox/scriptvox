//! Duration fitter (PLAN-R1 §5, CAP gap 2): narration fits a target length
//! before anything renders. Idea from OpenMontage #396 (the result was 1.94x
//! the target); the rules and code are ScriptVox's own.
//!
//! Ladder: keep (within 3%), `atempo` (within 8%), re-synthesize with the TTS
//! rate (within 50%), else stop and ask for a shorter or longer script.

/// Speaking rate for the estimate before TTS, words per second. Measured 2 Nov 2026 on the Edge and ElevenLabs
/// voices (speech time from the caption file): English 190 to 212 words a minute. Arabic on Edge, measured 6 Nov 2026
/// on 6 runs of 25 to 47 words: 6.8 to 7.5 letters a second, which is 1.5 of the words [`read_words`] counts. The open
/// voice (`selfhost`, Chatterbox) is faster, measured 3 and 4 Nov 2026: English about 245 a minute (98 words in 23.9 s of
/// speech over three runs on 4 Nov; one short run read 270), Arabic about 175.
/// The local voice `kokoro-rust` (af_heart) is slower, measured 13 Nov 2026: three plain texts of 58 words took
/// 20.16, 18.39 and 19.89 s, which is 2.88 to 3.15 words a second. Its estimate counts characters, see
/// [`spoken_words`].
pub fn words_per_s(lang: &str, engine: &str) -> f64 {
    // No voice: an easy reading pace for text on screen.
    if engine == "none" {
        return if lang == "ar" { 1.8 } else { 2.5 };
    }
    if engine == "kokoro-rust" && lang != "ar" {
        return KOKORO_RUST_WORDS_PER_S;
    }
    if let (Some(w), false) = (latin_words_per_s(lang), engine == "selfhost") {
        return w;
    }
    match (lang == "ar", engine == "selfhost") {
        (true, true) => 2.9,
        (true, false) => 1.5,
        (false, true) => 4.1,
        (false, false) => 3.3,
    }
}

/// Words a second of the Edge voices of five more languages (docs/specs/latin-languages.md). Measured 13 Nov 2026: three
/// plain texts of 35 to 45 words (12 to 20 s) on the two voices of each language, speech time from the first to the last
/// word event. Spanish 2.79 to 3.14, French 3.03 to 3.53, German 2.57 to 2.99, Portuguese 2.73 to 3.09. Indonesian is the
/// default voice (Ardi, 2.32 to 2.66); the other voice (Gadis) is slower, 1.78 to 2.05, and the fitter re-reads it.
fn latin_words_per_s(lang: &str) -> Option<f64> {
    Some(match lang {
        "es" => 3.0,
        "fr" => 3.3,
        "de" => 2.75,
        "pt" => 2.95,
        "id" => 2.5,
        _ => return None,
    })
}

/// The voice engine takes a speaking rate (Edge, ElevenLabs, the Rust Kokoro, the silent reading pace). The open voice
/// and the Kokoro plugin do not (the plugin speaks at its one speed): re-reading them at another rate gives the same
/// length, so the fitter goes straight to `atempo` or stops.
pub fn takes_rate(engine: &str) -> bool {
    !matches!(engine, "selfhost" | "kokoro")
}

/// The slowest and fastest rate an engine takes, in percent. ElevenLabs: its `speed` setting is 0.7 to 1.2 (its
/// text to speech guide, read 10 Nov 2026). The others take the whole re-read band.
pub fn rate_limits(engine: &str) -> (i32, i32) {
    if engine == "elevenlabs" {
        (-30, 20)
    } else {
        (-50, 50)
    }
}

/// Brief `voice_rate` (1.0 is the engine's own pace) as the whole percent the engine gets. No field, and 1.0, are
/// `None`: the same voice key and plan id as before the field. Past 0.75 or 1.3 a voice sounds dragged or rushed.
pub fn voice_rate_pct(rate: Option<f64>, engine: &str) -> Result<Option<i32>, String> {
    let Some(r) = rate else { return Ok(None) };
    if !(r.is_finite() && (0.75..=1.3).contains(&r)) {
        return Err(format!(
            "voice_rate is a number from 0.75 (slower) to 1.3 (faster), and 1.0 is the voice's own pace; it is not {r}"
        ));
    }
    let pct = ((r - 1.0) * 100.0).round() as i32;
    if pct == 0 {
        return Ok(None);
    }
    if !takes_rate(engine) {
        return Err(format!(
            "the {engine} voice takes no speaking rate, so voice_rate cannot change it: edge, elevenlabs, kokoro-rust and none take one"
        ));
    }
    let (lo, hi) = rate_limits(engine);
    if pct < lo || pct > hi {
        return Err(format!(
            "the {engine} voice takes voice_rate from {:.2} to {:.2}, not {r}",
            (1.0 + f64::from(lo) / 100.0).max(0.75),
            (1.0 + f64::from(hi) / 100.0).min(1.3)
        ));
    }
    Ok(Some(pct))
}

const KOKORO_RUST_WORDS_PER_S: f64 = 3.0;
/// Characters a second of `kokoro-rust` (af_heart), spaces and marks counted. Measured 13 Nov 2026 on four texts:
/// 14.4, 14.8, 15.3 and 14.6. The fourth had long words (56 words, 482 characters, 32.91 s): 1.70 words a second,
/// so a word count was wrong by half for it and the character count by 1 percent.
const KOKORO_RUST_CHARS_PER_S: f64 = 14.8;

/// [`read_words`] for the estimate of one engine. For `kokoro-rust` in English it is the characters as words of the
/// usual length (4.9 characters with the space), so `words / words_per_s` gives the measured seconds: with a plain
/// word count `plan` said "about 11 words short" for a text of long words that the voice then read 3.7 s over the
/// target. A number keeps the count of its spoken words. The other engines are measured in words and keep them.
pub fn spoken_words(text: &str, lang: &str, engine: &str) -> usize {
    if engine != "kokoro-rust" || lang == "ar" {
        return read_words(text, lang);
    }
    let per_word = KOKORO_RUST_CHARS_PER_S / KOKORO_RUST_WORDS_PER_S;
    text.split_whitespace()
        .map(|t| {
            if t.chars().any(|c| c.is_ascii_digit()) {
                read_units(t, lang).0 as f64
            } else {
                (t.chars().count() + 1) as f64 / per_word
            }
        })
        .sum::<f64>()
        .round() as usize
}

/// Written letters in one Arabic word as [`read_words`] counts it: Arabic word length varies more than its letter
/// rate, so 38 long words and 47 short ones took the same time (6 Nov 2026).
const AR_LETTERS_PER_WORD: f64 = 4.67;
const AR_LINE_LETTERS_PER_S: f64 = 7.3;

/// Words as the voice says them, for the estimates before TTS: a number is read as several words ("2005" is "two
/// thousand five", "1.4" is "one point four", "87%" is "eighty seven percent"). 4 Nov 2026: a 54-word script with
/// three years was estimated at 1.26x its target and read at 1.48x. Arabic counts letters, not written words, and its
/// whole numbers take about twice as long (3999 is 5 long words, 7.7 s for two of them on Edge).
/// ponytail: digit counts, not a number speller; good to a word or two per number.
pub fn read_words(text: &str, lang: &str) -> usize {
    let (counted, letters) = read_units(text, lang);
    counted + (letters as f64 / AR_LETTERS_PER_WORD).round() as usize
}

/// Seconds to read one dub line, for `plan`. Not [`read_words`]: a whole word of rounding is 5 percent of a short line
/// (eval promo line 3: 31 letters became 7 words, 1.36x, and the real need was 1.18x). A single Arabic line on Edge also
/// has no sentence pause inside it: 12 real lines on 6 Nov 2026 read 7.3 to 9.1 letters a second, so this takes the slow
/// end, against about 7 for a whole script.
pub fn line_seconds(text: &str, lang: &str, engine: &str) -> f64 {
    let (counted, letters) = read_units(text, lang);
    let wps = words_per_s(lang, engine);
    let letters_per_s = if takes_rate(engine) && engine != "none" {
        AR_LINE_LETTERS_PER_S
    } else {
        wps * AR_LETTERS_PER_WORD
    };
    counted as f64 / wps + letters as f64 / letters_per_s
}

/// Numbers and non-Arabic words as words, and the Arabic letters left to count.
fn read_units(text: &str, lang: &str) -> (usize, usize) {
    let ar = lang == "ar";
    // Spanish, French, German, Portuguese and Indonesian write "1.250" for a thousand and "3,5" for a decimal.
    let comma_decimal = latin_words_per_s(lang).is_some();
    let mut letters = 0usize;
    let counted: usize = text
        .split_whitespace()
        .map(|t| {
            let digit = |c: char| c.is_ascii_digit() || ('\u{0660}'..='\u{0669}').contains(&c);
            if !t.chars().any(digit) {
                if !ar {
                    // French sets "?", "!", ":" and "«" off with a space: a mark alone is not a word.
                    return usize::from(!comma_decimal || t.chars().any(char::is_alphanumeric));
                }
                // Not the short vowel marks: the voice reads them inside the letter's time.
                letters += t
                    .chars()
                    .filter(|&c| c.is_alphabetic() && !('\u{064B}'..='\u{065F}').contains(&c))
                    .count();
                return 0;
            }
            let (group, point) = if comma_decimal {
                ('.', ',')
            } else {
                (',', '.')
            };
            let t: String = t.chars().filter(|&c| c != group && c != '٬').collect();
            let (int, frac) = t.split_once([point, '٫']).unwrap_or((&t, ""));
            let n = int.chars().filter(|&c| digit(c)).count();
            let whole = match n {
                0..=2 => 1,
                3 | 4 => 3,
                _ => 2 * n.div_ceil(3),
            } * if ar { 2 } else { 1 };
            let f = frac.chars().filter(|&c| digit(c)).count();
            whole + if f > 0 { 1 + f } else { 0 } + t.contains(['%', '٪']) as usize
        })
        .sum();
    (counted, letters)
}

pub const KEEP: f64 = 0.03;
pub const TEMPO: f64 = 0.08;
pub const RESYNTH: f64 = 0.50;
/// A speaking-rate change beyond this sounds dragged or rushed (hand eval 4 Nov 2026: 27 words for 15 s read at
/// -40 percent): the fitter still fits, but `plan` and `make` warn.
pub const RATE_WARN: f64 = 0.15;

/// After a fit: a rate change past `RATE_WARN`, as the words to add (negative: to cut) so the voice reads near its
/// own speed.
pub fn rate_warn(pct: i32, words: usize) -> Option<i64> {
    let f = 1.0 + pct as f64 / 100.0;
    ((pct as f64 / 100.0).abs() > RATE_WARN && f > 0.0)
        .then(|| (words as f64 * (1.0 / f - 1.0)).round() as i64)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fit {
    Keep,
    /// `atempo` factor: above 1 speeds up.
    Tempo(f64),
    /// TTS rate in percent (Edge `rate`): above 0 speaks faster.
    Resynth(i32),
    /// Script too long: the words to cut (about).
    TooLong {
        ratio: f64,
        cut_words: usize,
    },
    /// Script too short: the words to add (about).
    TooShort {
        ratio: f64,
        add_words: usize,
    },
}

/// Decides how to fit `actual` seconds of narration into `target` seconds.
/// `words` and `lang` size the cut or add hint.
pub fn decide(actual: f64, target: f64, words: usize, wps: f64) -> Fit {
    let ratio = actual / target;
    let off = ratio - 1.0;
    if off.abs() <= KEEP {
        Fit::Keep
    } else if off.abs() <= TEMPO {
        Fit::Tempo(ratio)
    } else if off.abs() <= RESYNTH {
        Fit::Resynth((off * 100.0).round() as i32)
    } else {
        miss(actual, target, words, wps)
    }
}

/// The whole fit of `make`: the first read is `actual` seconds; when a re-read at another speaking rate is needed,
/// `reread` makes it and gives its length (`None`: it failed, and so does the ladder). Gives the rate of the re-read
/// (if one was made) and how the fit ended: `Keep`, `Tempo`, or a failure. `plan` runs it with the exact lengths of
/// `tts: none`, so the plan and the run give one answer (docs/specs/no-words.md).
pub fn ladder(
    actual: f64,
    target: f64,
    words: usize,
    wps: f64,
    rate_free: bool,
    (lo, hi): (i32, i32),
    reread: &mut dyn FnMut(i32) -> Option<f64>,
) -> Option<(Option<i32>, Fit)> {
    let tempo_or_miss = |len: f64| {
        if (len / target - 1.0).abs() <= 2.0 * TEMPO {
            Fit::Tempo(len / target)
        } else {
            miss(len, target, words, wps)
        }
    };
    Some(match decide(actual, target, words, wps) {
        // A voice without a rate, or with the pace the brief set: what a re-read would fix goes to atempo if it
        // is small enough.
        Fit::Resynth(_) if !rate_free => (None, tempo_or_miss(actual)),
        Fit::Resynth(pct) => {
            // No further than the engine goes: the rest is left to atempo, or the run stops.
            let pct = pct.clamp(lo, hi);
            let again = reread(pct)?;
            let d = match decide(again, target, words, wps) {
                // One TTS retry only: what is left goes to atempo if it is small enough.
                Fit::Resynth(_) => tempo_or_miss(again),
                d => d,
            };
            (Some(pct), d)
        }
        d => (None, d),
    })
}

/// The failure for a script that cannot fit, with a word-count hint.
pub fn miss(actual: f64, target: f64, words: usize, wps: f64) -> Fit {
    let ratio = actual / target;
    if ratio > 1.0 {
        let keep = (words as f64 / ratio).floor() as usize;
        Fit::TooLong {
            ratio,
            cut_words: words.saturating_sub(keep).max(1),
        }
    } else {
        let more = ((target - actual) * wps).ceil() as usize;
        Fit::TooShort {
            ratio,
            add_words: more.max(1),
        }
    }
}

/// The estimate before TTS. Only a clear miss stops the run here; the real
/// duration decides after TTS.
pub fn precheck(spoken_words: usize, pause_s: f64, target: f64, wps: f64) -> Option<Fit> {
    let est = spoken_words as f64 / wps + pause_s;
    match decide(est, target, spoken_words, wps) {
        f @ (Fit::TooLong { ratio, .. } | Fit::TooShort { ratio, .. })
            if !(0.55..=1.7).contains(&ratio) =>
        {
            Some(f)
        }
        _ => None,
    }
}

/// A plan-time warning. A voice without a rate fits only within twice the `atempo` band (16 percent), so a script
/// estimated outside it will likely stop after TTS (4 Nov 2026 eval: 0.75x and 0.81x, found only at `make`). A voice
/// with a rate fits, but past `RATE_WARN` it sounds dragged or rushed. Returns the miss with the words to add or cut.
pub fn plan_warn(
    spoken_words: usize,
    pause_s: f64,
    target: f64,
    wps: f64,
    rate_free: bool,
) -> Option<Fit> {
    if spoken_words == 0 {
        return None;
    }
    // The estimate is a few percent off the real read, so the plan warns from 80 percent of make's band (test 2,
    // 4 Nov 2026: 41 Arabic words for 20 s were estimated at +14, read at +16, and only make warned).
    let band = if rate_free {
        0.8 * RATE_WARN
    } else {
        2.0 * TEMPO
    };
    let est = spoken_words as f64 / wps + pause_s;
    ((est / target - 1.0).abs() > band).then(|| miss(est, target, spoken_words, wps))
}

/// `atempo` accepts 0.5 to 100 per instance; chain for anything below 0.5.
pub fn atempo_chain(factor: f64) -> String {
    let mut f = factor;
    let mut parts = Vec::new();
    while f < 0.5 {
        parts.push("atempo=0.5".to_string());
        f /= 0.5;
    }
    parts.push(format!("atempo={f:.6}"));
    parts.join(",")
}

/// A dub line: the voice alone may read this much faster before it borrows the gap after the line.
pub const LINE_RATE: f64 = 1.10;
/// A dub line never reads faster than this: past it the words are hard to follow. Change it here if listening says so.
pub const LINE_MAX: f64 = 1.25;
/// Silence kept before the next line.
const LINE_GAP_S: f64 = 0.05;

/// How one dub line fits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineFit {
    /// How much faster than its first read.
    pub speed: f64,
    /// Seconds it runs.
    pub run_s: f64,
}

/// `spoken` seconds in a source line of `slot` seconds with `room` seconds until the next line. Err: the speed it needs.
pub fn fit_line(spoken: f64, slot: f64, room: f64) -> Result<LineFit, f64> {
    let room = (room - LINE_GAP_S).max(slot);
    let run = (spoken / LINE_RATE).max(slot).min(spoken).min(room);
    let speed = spoken / run;
    if speed > LINE_MAX {
        return Err(speed);
    }
    Ok(LineFit { speed, run_s: run })
}

/// What fits at the top speed, in characters, from a measured or estimated read of `chars` characters.
pub fn line_budget(chars: usize, spoken: f64, room: f64) -> usize {
    ((chars as f64 * (room - LINE_GAP_S).max(0.0) * LINE_MAX / spoken.max(0.01)) as usize).max(1)
}

#[cfg(test)]
mod line_tests {
    use super::*;

    #[test]
    fn a_line_estimate_matches_the_real_read_of_the_eval_promo() {
        // Line 3 of the 6 Nov eval: 3.24 s, 3.48 s to the next line, real rate +18 percent. The plan said 1.36x.
        let line = "تعال قبل التاسعة، وفنجانك الأول مجاني.";
        let est = line_seconds(line, "ar", "edge");
        assert!((est - 4.25).abs() < 0.01, "{est}");
        let fit = fit_line(est, 3.24, 3.48).unwrap();
        assert!((fit.speed - 1.18).abs() < 0.07, "{}", fit.speed);
        // English has no letters to count, so the word rate stands.
        assert!((line_seconds("one two three", "en", "edge") - 3.0 / 3.3).abs() < 0.001);
    }

    #[test]
    fn a_line_fits_by_rate_then_the_gap_then_stops() {
        let keeps = fit_line(1.5, 2.0, 2.0).unwrap();
        assert_eq!((keeps.speed, keeps.run_s), (1.0, 1.5));
        let rate = fit_line(2.1, 2.0, 2.0).unwrap();
        assert!((rate.speed - 1.05).abs() < 0.001 && rate.run_s == 2.0);
        // 3.3 s of voice in a 2 s line: 10 percent faster gives 3 s, and the gap holds it.
        let borrow = fit_line(3.3, 2.0, 5.0).unwrap();
        assert!((borrow.speed - 1.1).abs() < 0.001 && (borrow.run_s - 3.0).abs() < 0.001);
        // The gap is short, so the rest goes to speed, up to the limit.
        let tight = fit_line(3.3, 2.0, 2.8).unwrap();
        assert!((tight.speed - 1.2).abs() < 0.001 && (tight.run_s - 2.75).abs() < 0.001);
        assert!(fit_line(5.0, 2.0, 2.5).is_err());
        // The new length at the top speed fits the room: 60 characters in 5 s, 2.45 s of room.
        assert_eq!(line_budget(60, 5.0, 2.5), 36);
    }
}

#[cfg(test)]
mod tests {
    const EN: f64 = 3.3;
    const AR: f64 = 1.8;
    use super::*;

    /// The five Latin languages: their own rate, "1.250" is one number, "3,5" is a decimal, a mark alone is no word.
    #[test]
    fn latin_languages_have_their_rate_and_their_numbers() {
        assert_eq!(words_per_s("es", "edge"), 3.0);
        assert_eq!(words_per_s("id", "edge"), 2.5);
        assert_eq!(words_per_s("fr", "none"), 2.5);
        assert_eq!(words_per_s("en", "edge"), 3.3);
        assert_eq!(read_words("Vraiment ? Oui !", "fr"), 2);
        assert_eq!(read_words("Really ? Yes !", "en"), 4);
        // "mil doscientos cincuenta" and "tres coma cinco".
        assert_eq!(read_words("1.250", "es"), 3);
        assert_eq!(read_words("3,5", "de"), 3);
        assert_eq!(read_words("1,250", "en"), 3);
        assert_eq!(read_words("3.5", "en"), 3);
    }

    #[test]
    fn ladder() {
        assert_eq!(decide(30.5, 30.0, 70, EN), Fit::Keep);
        assert_eq!(decide(31.8, 30.0, 70, EN), Fit::Tempo(1.06));
        assert_eq!(decide(36.0, 30.0, 70, EN), Fit::Resynth(20));
        assert_eq!(decide(25.5, 30.0, 70, EN), Fit::Resynth(-15));
    }

    #[test]
    fn the_om_396_case_stops_with_a_cut_hint() {
        // 1.94x the target: cut about half the words.
        match decide(58.2, 30.0, 140, EN) {
            Fit::TooLong { ratio, cut_words } => {
                assert!((ratio - 1.94).abs() < 1e-9);
                assert_eq!(cut_words, 68);
            }
            f => panic!("{f:?}"),
        }
    }

    #[test]
    fn numbers_count_as_the_words_they_are_read_as() {
        assert_eq!(read_words("In 2005 about 1.4 million", "en"), 9);
        assert_eq!(read_words("87% of trips, 12 roads", "en"), 6);
        assert_eq!(read_words("3,800,000 people", "en"), 7);
        assert_eq!(read_words("no numbers here", "en"), 3);
    }

    #[test]
    fn arabic_fits_the_real_runs_of_the_6_nov_eval() {
        // Real voice length on Edge, rate 0: 14.28 s, 27.82 s, 29.02 s. All three plans warned; none of the runs dragged.
        let cases = [
            ("الهاتف الجديد الآن بسعر 3999 درهمًا. بدلًا من 4599 درهمًا. اطلبه اليوم.", 15.0, 14.28),
            ("هذه واحة القهوة، المحمصة الجديدة على كورنيش أبوظبي. نشتري حبوبنا مباشرة من مزارعي اليمن وإثيوبيا، ونحمصها طازجة كل صباح. تعال قبل التاسعة، فكوبك الأول مجاني. اجلس قرب النافذة وشاهد القوارب. نفتح كل يوم من السابعة، بجوار الميناء القديم.", 30.0, 27.82),
            ("قبل النفط، كان رزق الناس هنا في قاع البحر. كان الغواصون يبحرون أشهرًا طويلة، لكنهم يغوصون بلا أجهزة، بحبل وحجر فقط. لذلك كانت اللؤلؤة الواحدة تكلف تعبًا كبيرًا، وكانت تصل إلى أسواق بعيدة. ثم جاء اللؤلؤ المزروع، فانتهى زمن الغوص، لكن أغانيه ما زالت تعيش في الإمارات.", 30.0, 29.02),
        ];
        let wps = words_per_s("ar", "edge");
        for (text, target, real) in cases {
            let n = read_words(text, "ar");
            let est = n as f64 / wps;
            assert!(
                (est / real - 1.0).abs() < 0.07,
                "{n} words, {est} s, real {real} s"
            );
            assert!(plan_warn(n, 0.0, target, wps, true).is_none(), "{text}");
        }
    }

    #[test]
    fn the_open_voice_reads_faster_and_takes_no_rate() {
        assert_eq!(words_per_s("en", "selfhost"), 4.1);
        // The local voice, measured 13 Nov 2026: 56 long words took 32.91 s, 58 plain words 20.16 s. The estimate
        // is within 5 percent of both; a word count at 3.3 a second gave 17.0 and 17.6 s.
        let est = |t: &str| {
            spoken_words(t, "en", "kokoro-rust") as f64 / words_per_s("en", "kokoro-rust")
        };
        let long = "Rayleigh scattering explains the colour of the atmosphere. Electromagnetic radiation from the sun contains every visible wavelength. Atmospheric molecules, especially nitrogen and oxygen, scatter shorter wavelengths approximately ten times more strongly than longer wavelengths. Consequently, scattered blue radiation reaches observers from every direction, while direct sunlight appears slightly yellower. Scattering intensity increases dramatically toward ultraviolet wavelengths.";
        let plain = "Why is the sky blue? Sunlight looks white, but it holds every colour. When it enters the air, it meets tiny molecules of gas. They scatter the short blue waves far more than the long red waves. So blue light comes to your eyes from every part of the sky, and the sky looks blue all day long.";
        assert!((est(long) / 32.91 - 1.0).abs() < 0.05, "{}", est(long));
        assert!((est(plain) / 20.16 - 1.0).abs() < 0.05, "{}", est(plain));
        // The other engines count words as before.
        assert_eq!(spoken_words(long, "en", "edge"), read_words(long, "en"));
        // 49 words for 14 s on the open voice: about 10.9 s estimated, add words.
        assert!(matches!(
            plan_warn(49, 0.0, 14.0, 4.5, false),
            Some(Fit::TooShort { add_words: 14, .. })
        ));
        assert!(plan_warn(60, 0.0, 14.0, 4.5, false).is_none());
        assert!(plan_warn(49, 0.0, 14.0, 3.3, true).is_none());
        // 27 words for 15 s on Edge (hand eval): about 8.2 s, the voice would read 40 percent slower.
        assert!(matches!(
            plan_warn(27, 0.0, 15.0, 3.3, true),
            Some(Fit::TooShort { add_words: 23, .. })
        ));
        assert_eq!(rate_warn(-40, 27), Some(18));
        assert_eq!(rate_warn(30, 52), Some(-12));
        assert_eq!(rate_warn(-15, 27), None);
        assert_eq!(words_per_s("ar", "edge"), 1.5);
        assert!(!takes_rate("selfhost") && takes_rate("edge") && takes_rate("elevenlabs"));
        assert!(!takes_rate("kokoro") && takes_rate("kokoro-rust") && takes_rate("none"));
        // 24 words at the open voice's rate are about 5.3 s: a 12 s target is a clear miss before TTS.
        assert!(precheck(24, 0.0, 12.0, words_per_s("en", "selfhost")).is_some());
        assert!(precheck(24, 0.0, 12.0, EN).is_none());
    }

    /// Brief `voice_rate`: whole percent for the engine, nothing at 1.0, and a refusal that names the range or the
    /// engines.
    #[test]
    fn voice_rate_is_a_percent_inside_the_range_of_the_engine() {
        assert_eq!(voice_rate_pct(None, "edge"), Ok(None));
        assert_eq!(voice_rate_pct(Some(1.0), "selfhost"), Ok(None));
        assert_eq!(voice_rate_pct(Some(1.004), "edge"), Ok(None));
        assert_eq!(voice_rate_pct(Some(0.85), "edge"), Ok(Some(-15)));
        assert_eq!(voice_rate_pct(Some(1.3), "kokoro-rust"), Ok(Some(30)));
        assert_eq!(voice_rate_pct(Some(0.75), "elevenlabs"), Ok(Some(-25)));
        assert_eq!(voice_rate_pct(Some(1.2), "elevenlabs"), Ok(Some(20)));
        let e = voice_rate_pct(Some(1.25), "elevenlabs").unwrap_err();
        assert!(e.contains("0.75 to 1.20"), "{e}");
        for bad in [0.74, 1.31, 0.0, -1.0, f64::NAN, f64::INFINITY] {
            let e = voice_rate_pct(Some(bad), "edge").unwrap_err();
            assert!(e.contains("0.75") && e.contains("1.3"), "{e}");
        }
        for engine in ["selfhost", "kokoro"] {
            let e = voice_rate_pct(Some(0.9), engine).unwrap_err();
            assert!(e.contains("edge, elevenlabs, kokoro-rust and none"), "{e}");
        }
    }

    #[test]
    fn precheck_stops_only_clear_misses() {
        assert!(precheck(180, 0.0, 30.0, EN).is_some()); // 54.5 s estimated
        assert!(precheck(100, 0.0, 30.0, EN).is_none()); // 30.3 s
        assert!(precheck(100, 0.0, 30.0, AR).is_some()); // 55.6 s: Arabic reads about half as many words
        assert!(matches!(
            precheck(5, 0.0, 30.0, EN),
            Some(Fit::TooShort { .. })
        ));
    }

    #[test]
    fn atempo_chains_below_half() {
        assert_eq!(atempo_chain(1.06), "atempo=1.060000");
        assert_eq!(atempo_chain(0.3), "atempo=0.5,atempo=0.600000");
    }
}
