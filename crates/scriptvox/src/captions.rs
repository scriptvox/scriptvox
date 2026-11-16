//! Captions (PLAN-R1 §5, RVX-007). Own schema: words with times come from the
//! TTS word events, pages are built by measured width, and ASS, SRT and VTT
//! are written from the same pages. ASS uses `WrapStyle: 2` with explicit `\N`,
//! so libass never re-wraps and the line breaks here are the final ones.

use harfrust::{Buffer, Font, ShapeOptions, ShaperFont};
use serde::{Deserialize, Serialize};

/// One spoken word. Times are seconds from the start of the video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// One caption page: up to `max_lines` lines, each a run of word indices.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Page {
    pub lines: Vec<Vec<usize>>,
    pub start: f64,
    /// When the page leaves the screen (held through a short pause, see `HOLD_S`).
    pub end: f64,
    /// When its last word is said: the gate measures this, not the hold.
    pub said_end: f64,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub max_width: f32,
    pub max_lines: usize,
    /// A silence longer than this starts a new page.
    pub max_gap: f64,
    /// A page never stays on screen longer than this (short-form readability).
    pub max_page: f64,
}

/// Measures text width in pixels at the caption size, with real shaping.
pub struct Measurer {
    font: Font,
    px_per_unit: f32,
    /// The font for the script this one lacks, and whether that script is Arabic.
    other: Option<(Box<Measurer>, bool)>,
}

const ARABIC_FAMILY: &str = "Noto Sans Arabic";

pub(crate) fn is_arabic(c: char) -> bool {
    matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{08A0}'..='\u{08FF}' | '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// True when the font has a glyph for `c`.
fn covers(font_bytes: &[u8], c: char) -> bool {
    use read_fonts::{FontRef, TableProvider};
    FontRef::new(font_bytes)
        .ok()
        .and_then(|f| f.cmap().ok()?.map_codepoint(c))
        .is_some_and(|g| g.to_u32() != 0)
}

impl Measurer {
    /// `size_px` is the ASS `Fontsize`. Libass (like VSFilter) maps that size to
    /// the OS/2 win ascent plus win descent, not to the em, so we scale the same way.
    pub fn new(font_bytes: Vec<u8>, size_px: f32) -> Option<Self> {
        // A line in two scripts is drawn with a second font, so it is measured with it (libass takes the same one).
        let other = match (covers(&font_bytes, 'a'), covers(&font_bytes, '\u{0627}')) {
            (true, false) => Some((ARABIC_FAMILY, true)),
            (false, true) => Some(("Noto Sans", false)),
            _ => None,
        }
        .and_then(|(family, arabic)| {
            let bytes = crate::template::font_bytes(family)?.to_vec();
            Some((Box::new(Self::one(bytes, size_px)?), arabic))
        });
        let mut m = Self::one(font_bytes, size_px)?;
        m.other = other;
        Some(m)
    }

    fn one(font_bytes: Vec<u8>, size_px: f32) -> Option<Self> {
        use read_fonts::{FontRef, TableProvider};
        let os2 = FontRef::new(&font_bytes).ok()?.os2().ok()?;
        let win = os2.us_win_ascent() as f32 + os2.us_win_descent() as f32;
        let font = Font::new(font_bytes, 0)?;
        Some(Self {
            font,
            px_per_unit: size_px / win,
            other: None,
        })
    }

    /// The width of `text`, each script with the font that draws it.
    pub fn width(&self, text: &str) -> f32 {
        let Some((other, other_arabic)) = &self.other else {
            return self.run(text);
        };
        let run = |t: &str, arabic: Option<bool>| {
            if arabic == Some(*other_arabic) {
                other.run(t)
            } else {
                self.run(t)
            }
        };
        let (mut sum, mut from, mut now) = (0.0, 0, None);
        for (i, c) in text.char_indices().filter(|(_, c)| c.is_alphabetic()) {
            let arabic = is_arabic(c);
            if now.is_some_and(|n| n != arabic) {
                sum += run(&text[from..i], now);
                from = i;
            }
            now = Some(arabic);
        }
        sum + run(&text[from..], now)
    }

    fn run(&self, text: &str) -> f32 {
        let shaper = ShaperFont::new(&self.font);
        let mut buf = Buffer::new();
        buf.push_str(text);
        buf.guess_segment_properties();
        if harfrust::shape(&shaper, &mut buf, ShapeOptions::new()).is_err() {
            // ponytail: an unshapeable run measures zero. The pager cannot see it, and `gates::text_bounds` says it was not measured.
            return 0.0;
        }
        let units: i32 = buf.glyph_positions().iter().map(|p| p.x_advance).sum();
        units as f32 * self.px_per_unit
    }
}

/// Letters and digits only, lower case: the key that matches a TTS word to a script word.
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// An event that holds several words (Edge sends "In 2005" as one, 4 Nov 2026) becomes one event per word, its time
/// shared by letter count. Without this the script word "In" took the whole event and every later word shifted by
/// one: a word held 3.8 s on screen and the last page had no time at all.
fn split_events(events: &[Word]) -> Vec<Word> {
    let mut out = Vec::with_capacity(events.len());
    for e in events {
        let parts: Vec<&str> = e.text.split_whitespace().collect();
        if parts.len() < 2 {
            out.push(e.clone());
            continue;
        }
        let total: usize = parts.iter().map(|p| p.chars().count()).sum();
        let mut t = e.start;
        for p in parts {
            let d = (e.end - e.start) * p.chars().count() as f64 / total.max(1) as f64;
            out.push(Word {
                text: p.to_string(),
                start: t,
                end: t + d,
            });
            t += d;
        }
    }
    out
}

/// TTS word events lose punctuation and may split or respell a word ("2026" becomes "twenty twenty-six"), and speech
/// to text writes "one point four" as "1 .4". Put the script's own words back on the times: the longest common
/// run of words stays fixed (one miss must not shift every later word, 6 Nov 2026: 2.9 s), the words between two
/// fixed ones share the events between them.
/// ponytail: a full table, 160 MB at 9000 words a side; a band if a script gets that long.
pub fn align(script: &str, events: &[Word]) -> Vec<Word> {
    let events = &split_events(events);
    let tokens: Vec<&str> = script.split_whitespace().collect();
    let a: Vec<String> = tokens.iter().map(|t| norm(t)).collect();
    let b: Vec<String> = events.iter().map(|e| norm(&e.text)).collect();
    let (n, m) = (a.len(), b.len());
    let same = |i: usize, j: usize| !a[i].is_empty() && a[i] == b[j];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    let mut table = vec![0u16; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if same(i, j) {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let mut out: Vec<Word> = Vec::with_capacity(n);
    let (mut i, mut j) = (0, 0);
    let (mut gap_i, mut gap_j) = (0, 0);
    while i < n || j < m {
        let hit = i < n && j < m && same(i, j) && table[at(i, j)] == table[at(i + 1, j + 1)] + 1;
        if hit {
            fill_gap(
                &tokens[gap_i..i],
                &events[gap_j..j],
                events.get(j),
                &mut out,
            );
            out.push(Word {
                text: tokens[i].to_string(),
                start: events[j].start,
                end: events[j].end,
            });
            (i, j) = (i + 1, j + 1);
            (gap_i, gap_j) = (i, j);
        } else if j == m || (i < n && table[at(i + 1, j)] >= table[at(i, j + 1)]) {
            i += 1;
        } else {
            j += 1;
        }
    }
    fill_gap(&tokens[gap_i..n], &events[gap_j..m], None, &mut out);
    out
}

/// The script words between two fixed words, on the events between them: one to one when the counts agree (misheard
/// words), else shared by letters. Events with no word left stretch the word before them. Words with no event left
/// share the time up to `next`.
fn fill_gap(tokens: &[&str], events: &[Word], next: Option<&Word>, out: &mut Vec<Word>) {
    if tokens.len() == events.len() {
        out.extend(tokens.iter().zip(events).map(|(t, e)| Word {
            text: t.to_string(),
            start: e.start,
            end: e.end,
        }));
        return;
    }
    let before = out.last().map_or(0.0, |w| w.end);
    let (start, end) = match (events.first(), events.last()) {
        (Some(a), Some(b)) => (a.start, b.end),
        _ => (before, next.map_or(before, |w| w.start)),
    };
    if tokens.is_empty() {
        if let Some(w) = out.last_mut() {
            w.end = end;
        }
        return;
    }
    let total: usize = tokens.iter().map(|t| t.chars().count()).sum();
    let mut t = start;
    for tok in tokens {
        let d = (end - start) * tok.chars().count() as f64 / total.max(1) as f64;
        out.push(Word {
            text: tok.to_string(),
            start: t,
            end: t + d,
        });
        t += d;
    }
}

/// How many em fit in the ASS `Fontsize` of a font: libass maps the size to the
/// OS/2 win ascent plus descent, so a font with tall win metrics looks smaller.
pub fn em_factor(font_bytes: &[u8]) -> Option<f32> {
    use read_fonts::{FontRef, TableProvider};
    let f = FontRef::new(font_bytes).ok()?;
    let os2 = f.os2().ok()?;
    let upem = f.head().ok()?.units_per_em() as f32;
    Some((os2.us_win_ascent() as f32 + os2.us_win_descent() as f32) / upem)
}

/// Sentence ends in Latin and Arabic script: a page never runs past one. A short form with a period is not an end:
/// an initial ("W. H. Auden"), letters with periods ("U.S.", "e.g.") and a title ("Dr.", "Mr.").
pub fn ends_sentence(w: &str) -> bool {
    let w = w.trim_end_matches(['"', '\'', ')', '»', '”']);
    w.ends_with(['.', '!', '?', '؟', '۔']) && !short_form(w)
}

/// "W.", "U.S.", "e.g.", "Dr.": a period that belongs to the word. ASCII letters only, so no other script changes.
/// ponytail: a closed list of titles, and a sentence that ends in one capital letter ("vitamin C.") or in "U.S."
/// runs on into the next page; the width break still cuts it. Look at the next word's capital when a review shows it.
fn short_form(w: &str) -> bool {
    const TITLES: &[&str] = &[
        "Dr", "Mr", "Mrs", "Ms", "Prof", "Sr", "Jr", "St", "Mt", "vs",
    ];
    let Some(body) = w
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .strip_suffix('.')
    else {
        return false;
    };
    let letter = |p: &str| p.len() == 1 && p.as_bytes()[0].is_ascii_alphabetic();
    let mut parts = body.split('.');
    match (parts.next(), parts.clone().next()) {
        // One letter: an initial when it is a capital ("a." and "I." at a sentence end are rare; "I." is kept an end).
        (Some(p), None) => {
            (letter(p) && p != "I" && p.as_bytes()[0].is_ascii_uppercase()) || TITLES.contains(&p)
        }
        // "U.S", "e.g", "a.m": every piece is one letter.
        (Some(p), Some(_)) => letter(p) && parts.all(letter),
        _ => false,
    }
}

thread_local! {
    /// True while [`paginate`] works on words in Spanish, French, German, Portuguese or Indonesian: only then do the
    /// page rules of those languages apply, so English and Arabic pages are what they were.
    // ponytail: a flag for the time of one `paginate` call, in place of a parameter through eight helpers; make it a
    // parameter when the pager gets a language of its own.
    static LATIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A word that leaves the reader hanging at the end of a page ("to", "the", "في"). A word with punctuation after it
/// ends a phrase, so it is never weak.
fn weak(w: &str) -> bool {
    // A bare symbol ("≈", "→", "≤") always leads into what follows.
    if w.chars()
        .all(|c| ('\u{2190}'..='\u{2BFF}').contains(&c) || c == '×' || c == '±')
    {
        return true;
    }
    if w.ends_with(|c: char| !c.is_alphanumeric()) {
        return false;
    }
    let written = w;
    let w = w.to_lowercase();
    // ponytail: a short closed list per language; add words when a review shows a bad page end.
    const WEAK: &[&str] = &[
        "a",
        "an",
        "the",
        "to",
        "of",
        "in",
        "on",
        "at",
        "for",
        "from",
        "with",
        "by",
        "and",
        "or",
        "but",
        "as",
        "into",
        "than",
        "that",
        "is",
        "are",
        "was",
        "were",
        "be",
        "my",
        "your",
        "our",
        "their",
        "his",
        "her",
        "its",
        "this",
        "these",
        "those",
        "no",
        "not",
        "very",
        "only",
        "about",
        "around",
        "because",
        "when",
        "if",
        "نحو",
        "حوالي",
        "قرابة",
        "في",
        "من",
        "على",
        "إلى",
        "الى",
        "عن",
        "مع",
        "أن",
        "التي",
        "الذي",
        "الذين",
        "أو",
        "لكن",
        "ثم",
        "بين",
        "حتى",
        "منذ",
        "لأن",
        "لان",
        "لكي",
        "كي",
        "عندما",
        "إذا",
        "اذا",
        "حيث",
    ];
    // Spanish, French, German, Portuguese, Indonesian (docs/specs/latin-languages.md): read only for a script in one
    // of them. A word in capitals ("MIT") is a name.
    const WEAK_LATIN: &[&str] = &[
        "el", "los", "las", "un", "una", "unos", "unas", "de", "del", "en", "por", "para", "que",
        "es", "su", "sus", "muy", "pero", "como", "cuando", "porque", "si", "sobre", "entre",
        "hasta", "desde", "hacia", "le", "la", "les", "une", "des", "du", "et", "ou", "à", "au",
        "aux", "dans", "sur", "avec", "qui", "est", "sont", "ne", "ce", "cette", "ces", "sa",
        "ses", "très", "mais", "quand", "chez", "vers", "sous", "comme", "der", "das", "und",
        "ein", "eine", "einen", "mit", "auf", "für", "zu", "von", "im", "ist", "sind", "nicht",
        "oder", "aber", "bei", "zum", "zur", "dem", "wenn", "weil", "dass", "sehr", "nur", "os",
        "uma", "da", "dos", "em", "na", "com", "sem", "mas", "se", "não", "muito", "só", "ao",
        "às", "é", "são", "quando", "yang", "dan", "di", "ke", "dari", "untuk", "dengan", "pada",
        "atau", "tetapi", "karena", "adalah", "tidak", "sangat", "hanya", "sebuah", "oleh",
        "dalam",
    ];
    let name = written.chars().count() > 1 && !written.chars().any(char::is_lowercase);
    WEAK.contains(&w.as_str()) || (LATIN.get() && !name && WEAK_LATIN.contains(&w.as_str()))
}

/// A mark that French sets off with a space ("Vraiment ?", "mot »"): it stays with the word before it.
fn closes(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| "!?:;»%".contains(c))
}

/// An opening mark set off with a space ("« mot"): it stays with the word after it.
fn opens(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| "«¿¡".contains(c))
}

/// A word that only makes sense after the number before it ("four thousand", "أربعة آلاف"): a page must not
/// start with it.
fn number_tail(w: &str) -> bool {
    let w = w
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    const TAIL: &[&str] = &[
        "hundred",
        "thousand",
        "million",
        "billion",
        "percent",
        "مائة",
        "مئة",
        "ألف",
        "آلاف",
        "الاف",
        "مليون",
        "ملايين",
        "مليار",
        "مليارات",
        "بالمئة",
        "بالمائة",
    ];
    // The five Latin languages, read only for a script in one of them (see `LATIN`).
    const TAIL_LATIN: &[&str] = &[
        "mil",
        "millones",
        "millón",
        "ciento",
        "mille",
        "milliards",
        "tausend",
        "hundert",
        "millionen",
        "milliarden",
        "prozent",
        "milhões",
        "milhão",
        "bilhões",
        "ribu",
        "juta",
        "ratus",
        "miliar",
        "persen",
    ];
    TAIL.contains(&w.as_str()) || (LATIN.get() && TAIL_LATIN.contains(&w.as_str()))
}

/// If the words moved to the next page start with "thousand", the number before it goes with them.
/// Then, if the page now ends on a weak word ("about"), that goes too.
fn keep_number(from: &mut Vec<usize>, carry: &mut Vec<usize>, words: &[Word]) {
    if carry.first().is_some_and(|&j| number_tail(&words[j].text)) && from.len() > 1 {
        carry.insert(0, from.pop().unwrap());
    }
    if !carry.is_empty() {
        let mut more = take_weak_tail(from, words);
        more.append(carry);
        *carry = more;
    }
}

/// A word that leans on the word before it, so a line never breaks just before it: a lone letter read as a
/// variable ("pi r squared", "x"), and "squared" or "cubed". "a" and "I" are ordinary words.
fn leans_back(w: &str) -> bool {
    let w = w
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    let mut c = w.chars();
    match (c.next(), c.next()) {
        (Some(l), None) => l.is_ascii_lowercase() && l != 'a' && l != 'i',
        _ => matches!(w.as_str(), "squared" | "cubed"),
    }
}

/// Moves weak words at the end of `line` into `carry` (at most two, and never the line's only word).
fn take_weak_tail(line: &mut Vec<usize>, words: &[Word]) -> Vec<usize> {
    let mut carry = Vec::new();
    while carry.len() < 2 && line.len() > 1 && weak(&words[*line.last().unwrap()].text) {
        carry.insert(0, line.pop().unwrap());
    }
    carry
}

/// Greedy pager: fill a line until the next word would not fit, fill a page
/// until `max_lines`, and start a new page at a sentence end or a long gap.
/// A single word wider than the line gets a line of its own.
pub fn paginate(words: &[Word], layout: &Layout, width: impl Fn(&str) -> f32) -> Vec<Page> {
    let all = words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    LATIN.set(!matches!(crate::make::detect_lang(&all), "en" | "ar"));
    let mut pages: Vec<Page> = Vec::new();
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut line: Vec<usize> = Vec::new();

    let text = |idx: &[usize]| {
        idx.iter()
            .map(|&i| words[i].text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let flush = |lines: &mut Vec<Vec<usize>>, line: &mut Vec<usize>, pages: &mut Vec<Page>| {
        if !line.is_empty() {
            lines.push(std::mem::take(line));
        }
        // No lone last word on a page's last line, however the page ends:
        // borrow one word from the line above.
        let n = lines.len();
        if n >= 2 && lines[n - 1].len() == 1 && lines[n - 2].len() > 2 {
            let moved = lines[n - 2].pop().unwrap();
            lines[n - 1].insert(0, moved);
        }
        if !lines.is_empty() {
            let first = lines[0][0];
            let last = *lines.last().unwrap().last().unwrap();
            pages.push(Page {
                lines: std::mem::take(lines),
                start: words[first].start,
                end: words[last].end,
                said_end: words[last].end,
            });
        }
    };

    for (i, w) in words.iter().enumerate() {
        if let Some(&prev) = line.last().or_else(|| lines.last().and_then(|l| l.last())) {
            let first = lines
                .first()
                .or(Some(&line))
                .and_then(|l| l.first())
                .copied()
                .unwrap_or(prev);
            if w.start - words[prev].end > layout.max_gap {
                flush(&mut lines, &mut line, &mut pages);
            } else if w.end - words[first].start > layout.max_page {
                // A page that ends by time does not end on "to" or "the": those words open the next page.
                // A line made only of weak words goes whole, if the page keeps a line above it.
                let mut carry = if !line.is_empty()
                    && !lines.is_empty()
                    && line.iter().all(|&j| weak(&words[j].text))
                {
                    std::mem::take(&mut line)
                } else if !line.is_empty() {
                    take_weak_tail(&mut line, words)
                } else {
                    lines
                        .last_mut()
                        .map(|l| take_weak_tail(l, words))
                        .unwrap_or_default()
                };
                // No orphans here either: a sentence's last word never opens a page alone, and a number keeps
                // its "thousand".
                if carry.is_empty() && (ends_sentence(&w.text) || number_tail(&w.text)) {
                    let tail = if line.is_empty() {
                        lines.last_mut()
                    } else {
                        Some(&mut line)
                    };
                    if let Some(t) = tail.filter(|t| t.len() > 1) {
                        carry.extend(t.pop());
                    }
                }
                let from = if line.is_empty() {
                    lines.last_mut()
                } else {
                    Some(&mut line)
                };
                if let Some(f) = from {
                    keep_number(f, &mut carry, words);
                }
                flush(&mut lines, &mut line, &mut pages);
                line.extend(carry);
            }
        }
        if !line.is_empty() {
            let mut trial = line.clone();
            trial.push(i);
            if width(&text(&trial)) > layout.max_width {
                lines.push(std::mem::take(&mut line));
                // "pi | r squared": the words the next line leans on go down with it (at most two).
                let mut lean: Vec<usize> = Vec::new();
                let mut lead = i;
                while lean.len() < 2
                    && leans_back(&words[lead].text)
                    && lines.last().is_some_and(|l| l.len() > 1)
                {
                    lead = lines.last_mut().unwrap().pop().unwrap();
                    lean.insert(0, lead);
                }
                if lines.len() < layout.max_lines {
                    line.extend(lean);
                } else {
                    // No orphans: if this word ends the sentence, it would sit alone
                    // on the next page, so the page gives it its last word as company.
                    // A page that ends on a weak word ("channels to") also hands it on.
                    let orphan = (ends_sentence(&w.text) || number_tail(&w.text))
                        && lines.last().is_some_and(|l| l.len() > 1);
                    let mut carry = lines
                        .last_mut()
                        .map(|l| take_weak_tail(l, words))
                        .unwrap_or_default();
                    carry.append(&mut lean);
                    if orphan && carry.is_empty() {
                        // The company is one word, or a group that leans back ("pi r squared"), and only if the
                        // line keeps a word.
                        let l = lines.last_mut().unwrap();
                        let mut k = 1;
                        while k < l.len() && leans_back(&words[l[l.len() - k]].text) {
                            k += 1;
                        }
                        if k < l.len() {
                            carry = l.split_off(l.len() - k);
                        }
                    }
                    if let Some(f) = lines.last_mut() {
                        keep_number(f, &mut carry, words);
                    }
                    flush(&mut lines, &mut line, &mut pages);
                    line.extend(carry);
                }
            }
        }
        line.push(i);
        if ends_sentence(&w.text) {
            flush(&mut lines, &mut line, &mut pages);
        }
    }
    flush(&mut lines, &mut line, &mut pages);
    no_short_tail(&mut pages, words, layout, &width);
    no_flash(&mut pages, words, layout, &width);
    if LATIN.get() {
        keep_marks(&mut pages, words);
    }
    LATIN.set(false);
    hold_through_pauses(&mut pages);
    one_at_a_time(&mut pages);
    pages
}

/// No page is on screen with the next one, whatever the word times are: a page ends when the next one starts at
/// the latest. Two pages at one time are drawn one above the other, and the upper one can lie on a card (13 Nov
/// 2026, word times that ran past the end of a scene). The gate `captions` fails on such a pair.
fn one_at_a_time(pages: &mut [Page]) {
    for i in 1..pages.len() {
        let next = pages[i].start;
        let p = &mut pages[i - 1];
        p.end = p.end.min(next).max(p.start);
        p.said_end = p.said_end.min(p.end);
    }
}

/// French sets "?", "!", ":", ";" and the quote marks off with a space, so each is a word of its own here. A closing
/// mark at the start of a line goes up to the word before it, and an opening mark at the end of a line goes down to
/// the word after it, also from page to page. A mark is one or two characters: the line stays inside its margin.
fn keep_marks(pages: &mut Vec<Page>, words: &[Word]) {
    if !words.iter().any(|w| closes(&w.text) || opens(&w.text)) {
        return;
    }
    // (page, line) of each line in reading order.
    let at = |pages: &[Page]| -> Vec<(usize, usize)> {
        pages
            .iter()
            .enumerate()
            .flat_map(|(p, pg)| (0..pg.lines.len()).map(move |l| (p, l)))
            .collect()
    };
    let lines = at(pages);
    let mut changed = vec![false; pages.len()];
    for k in 1..lines.len() {
        let ((pp, pl), (np, nl)) = (lines[k - 1], lines[k]);
        while pages[np].lines[nl]
            .first()
            .is_some_and(|&i| closes(&words[i].text))
        {
            let i = pages[np].lines[nl].remove(0);
            pages[pp].lines[pl].push(i);
            (changed[pp], changed[np]) = (true, true);
        }
        while pages[pp].lines[pl]
            .last()
            .is_some_and(|&i| opens(&words[i].text))
            && !pages[np].lines[nl].is_empty()
        {
            let i = pages[pp].lines[pl].pop().unwrap();
            pages[np].lines[nl].insert(0, i);
            (changed[pp], changed[np]) = (true, true);
        }
    }
    for (p, _) in pages.iter_mut().zip(changed).filter(|(_, c)| *c) {
        p.lines.retain(|l| !l.is_empty());
        if let (Some(&first), Some(&last)) = (
            p.lines.first().and_then(|l| l.first()),
            p.lines.last().and_then(|l| l.last()),
        ) {
            p.start = words[first].start;
            p.end = words[last].end;
            p.said_end = words[last].end;
        }
    }
    pages.retain(|p| !p.lines.is_empty());
}

/// A page shorter than this cannot be read.
const FLASH_S: f64 = 0.25;

/// A very short word alone on a page flashes by (Arabic "من", 0.16 s, 5 Nov 2026, gate `captions`): it joins the next
/// page's first line when that still fits and no pause lies between, else the last line of the page before.
fn no_flash(pages: &mut Vec<Page>, words: &[Word], layout: &Layout, width: &impl Fn(&str) -> f32) {
    let text = |l: &[usize]| {
        l.iter()
            .map(|&i| words[i].text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut k = 0;
    while k < pages.len() {
        if pages[k].end - pages[k].start >= FLASH_S || pages.len() < 2 {
            k += 1;
            continue;
        }
        let mine: Vec<usize> = pages[k].lines.concat();
        if k + 1 < pages.len() && pages[k + 1].start - pages[k].end <= layout.max_gap {
            let mut joined = mine.clone();
            joined.extend(&pages[k + 1].lines[0]);
            if width(&text(&joined)) <= layout.max_width {
                pages[k + 1].lines[0] = joined;
                pages[k + 1].start = pages[k].start;
                pages.remove(k);
                continue;
            }
        }
        if k > 0 && pages[k].start - pages[k - 1].end <= layout.max_gap {
            let last = pages[k - 1].lines.last().unwrap();
            let mut joined = last.clone();
            joined.extend(&mine);
            if width(&text(&joined)) <= layout.max_width {
                *pages[k - 1].lines.last_mut().unwrap() = joined;
                pages[k - 1].end = pages[k].end;
                pages[k - 1].said_end = pages[k].said_end;
                pages.remove(k);
                continue;
            }
            // Neither side has room ("Most floods start" | "with" | "a blocked drain.", pop, 5 Nov 2026): the page before
            // hands over its last word, which gives this page the time to be read.
            let last = pages[k - 1].lines.last().unwrap();
            if last.len() >= 2 {
                let moved = last[last.len() - 1];
                let mut joined = vec![moved];
                joined.extend(&mine);
                // "r squared" and "5 thousand" stay together.
                let tied = leans_back(&words[moved].text) || number_tail(&words[moved].text);
                if !tied && width(&text(&joined)) <= layout.max_width {
                    let kept = last[last.len() - 2];
                    pages[k - 1].lines.last_mut().unwrap().pop();
                    pages[k - 1].end = words[moved].start;
                    pages[k - 1].said_end = words[kept].end;
                    pages[k].start = words[moved].start;
                    pages[k].lines = vec![joined];
                }
            }
        }
        k += 1;
    }
}

/// The note that scriptvox wrote this content at this path: `.scriptvox/sidecars/<hash of the path>` holds the hash.
fn sidecar_note(path: &std::path::Path) -> std::path::PathBuf {
    let full = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let name = blake3::hash(full.to_string_lossy().as_bytes()).to_hex();
    std::path::Path::new(".scriptvox/sidecars").join(name.as_str())
}

/// The file at `path` is the one scriptvox wrote there, not changed since.
fn sidecar_is_ours(path: &std::path::Path) -> bool {
    match (
        std::fs::read(path),
        std::fs::read_to_string(sidecar_note(path)),
    ) {
        (Ok(now), Ok(note)) => blake3::hash(&now).to_hex().as_str() == note.trim(),
        _ => false,
    }
}

/// Copies captions.srt, captions.vtt and credits.txt from `dir` beside `video`, under the video's name. `keep`: the user
/// chose that name, so a different file already there is theirs, unless scriptvox wrote it: the new one takes the next
/// free number, with a warning. `artifacts.sidecar_srt`, `sidecar_vtt` and `credits` say where each file landed.
pub fn sidecars(
    dir: &std::path::Path,
    video: &std::path::Path,
    keep: bool,
    env: &mut crate::envelope::Envelope,
) {
    for (from, ext) in [
        ("captions.srt", "srt"),
        ("captions.vtt", "vtt"),
        ("credits.txt", "credits.txt"),
    ] {
        let Ok(new) = std::fs::read(dir.join(from)) else {
            continue;
        };
        // A video with no words has no cue: no empty caption file beside it.
        if ext != "credits.txt" && !new.windows(3).any(|w| w == b"-->") {
            continue;
        }
        let first = video.with_extension(ext);
        let stem = video
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let to = (1..)
            .map(|n| match n {
                1 => first.clone(),
                n => video.with_file_name(format!("{stem}-{n}.{ext}")),
            })
            .find(|p| {
                !keep
                    || !p.exists()
                    || std::fs::read(p).is_ok_and(|old| old == new)
                    || sidecar_is_ours(p)
            })
            .unwrap();
        let written = std::fs::write(&to, &new).is_ok();
        if written {
            let note = sidecar_note(&to);
            let _ = note.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(note, blake3::hash(&new).to_hex().as_str());
            // Where each file landed, in every result: the run folder's `captions_srt` alone made a reader think
            // the child of a `resume` had no captions beside its video (T8, 13 Nov 2026).
            let abs = std::path::absolute(&to).unwrap_or_else(|_| to.clone());
            let name = if ext == "credits.txt" {
                "credits".to_string()
            } else {
                format!("sidecar_{ext}")
            };
            env.artifacts.insert(name, serde_json::json!(abs));
        }
        if written && to != first {
            let to = std::path::absolute(&to).unwrap_or(to);
            env.findings.push(serde_json::json!({"gate": "sidecar", "status": "warn",
                "detail": format!("{} was already there and stays as it was: the new captions are in {}", first.display(), to.display()),
                "fix": "Compare the two files and keep the one you want."}));
        }
    }
}

/// A sentence's last one or two words never stand alone on a page that the voice reaches without a pause ("…gives
/// the heat back" | "all night.", 4 Nov 2026 eval): the page before hands over the end of its last line, up to
/// two words, while that line keeps two and the new line still fits.
fn no_short_tail(
    pages: &mut [Page],
    words: &[Word],
    layout: &Layout,
    width: &impl Fn(&str) -> f32,
) {
    for k in 1..pages.len() {
        let (head, tail) = pages.split_at_mut(k);
        let (prev, cur) = (&mut head[k - 1], &mut tail[0]);
        let n: usize = cur.lines.iter().map(Vec::len).sum();
        let last = *cur.lines[0].last().unwrap();
        if n > 2
            || cur.lines.len() != 1
            || !ends_sentence(&words[last].text)
            || cur.start - prev.said_end > HOLD_S
        {
            continue;
        }
        let Some(pl) = prev.lines.last_mut() else {
            continue;
        };
        // The fewest words (1 or 2) that give the page 3, keep 2 on the line above, and do not leave that line on
        // "the" or "من". No such move: the page stays as it is.
        let first = cur.lines[0][0];
        let ok = |m: usize| {
            if pl.len() < m + 2 || weak(&words[pl[pl.len() - m - 1]].text) {
                return false;
            }
            let moved = &pl[pl.len() - m..];
            let mut trial = moved.to_vec();
            trial.extend(&cur.lines[0]);
            let text = trial
                .iter()
                .map(|&i| words[i].text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            // Keep a lean group ("pi r squared") whole and a page ended by time inside its time.
            !moved
                .iter()
                .any(|&w| ends_sentence(&words[w].text) || leans_back(&words[w].text))
                && !leans_back(&words[first].text)
                && width(&text) <= layout.max_width
                && words[last].end - words[moved[0]].start <= layout.max_page
        };
        let moved = (1..=2).find(|&m| n + m >= 3 && ok(m)).unwrap_or(0);
        if moved > 0 {
            let mut line = pl.split_off(pl.len() - moved);
            line.extend(&cur.lines[0]);
            cur.lines[0] = line;
        }
        if moved > 0 {
            let pe = *pl.last().unwrap();
            prev.end = words[pe].end;
            prev.said_end = words[pe].end;
            cur.start = words[cur.lines[0][0]].start;
        }
    }
}

/// A page stays on screen through a short pause until the next page starts, so the picture never flashes empty
/// between two sentences (judge eval of 4 Nov: a frame in a 0.4 s pause scored 1, "empty frame"). A longer pause
/// still clears the screen.
pub const HOLD_S: f64 = 0.8;

fn hold_through_pauses(pages: &mut [Page]) {
    for i in 1..pages.len() {
        let next = pages[i].start;
        let p = &mut pages[i - 1];
        if next > p.end && next - p.end <= HOLD_S {
            p.end = next;
        }
    }
}

/// A right-to-left mark for a line with Arabic letters. Renderers and players pick a line's direction from its
/// start, so a line that opens with a number ("2026 بسعر") would otherwise be laid out left to right.
pub fn rtl_mark(text: &str) -> &'static str {
    if text.chars().any(is_arabic) {
        "\u{200F}"
    } else {
        ""
    }
}

/// True when the whole script is mostly Arabic. A page takes its direction from this, not from one Arabic word.
fn rtl_script(words: &[Word]) -> bool {
    let letters = || {
        words
            .iter()
            .flat_map(|w| w.text.chars())
            .filter(|c| c.is_alphabetic())
    };
    letters().filter(|c| is_arabic(*c)).count() * 2 >= letters().count()
}

/// The direction mark of a caption line: none without Arabic, else the direction of the script around it.
fn page_mark(line: &str, rtl: bool) -> &'static str {
    match (rtl_mark(line).is_empty(), rtl) {
        (true, _) => "",
        (false, true) => "\u{200F}",
        (false, false) => "\u{200E}",
    }
}

fn page_text(p: &Page, words: &[Word], sep: &str, rtl: bool) -> String {
    p.lines
        .iter()
        .map(|l| {
            let line = l
                .iter()
                .map(|&i| words[i].text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            format!("{}{line}", page_mark(&line, rtl))
        })
        .collect::<Vec<_>>()
        .join(sep)
}

fn clock(t: f64, frac_sep: char, cs: bool) -> String {
    // Round once in the output unit, so 3.456 s is 3.46 in ASS and 3,456 in SRT.
    let per_s: u64 = if cs { 100 } else { 1000 };
    let n = (t.max(0.0) * per_s as f64).round() as u64;
    let (s, f) = (n / per_s, n % per_s);
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if cs {
        format!("{h}:{m:02}:{s:02}{frac_sep}{f:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}{frac_sep}{f:03}")
    }
}

pub fn to_srt(pages: &[Page], words: &[Word]) -> String {
    pages
        .iter()
        .enumerate()
        .map(|(n, p)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                n + 1,
                clock(p.start, ',', false),
                clock(p.end, ',', false),
                page_text(p, words, "\n", rtl_script(words))
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn to_vtt(pages: &[Page], words: &[Word]) -> String {
    let cues: Vec<String> = pages
        .iter()
        .map(|p| {
            format!(
                "{} --> {}\n{}\n",
                clock(p.start, '.', false),
                clock(p.end, '.', false),
                page_text(p, words, "\n", rtl_script(words))
            )
        })
        .collect();
    format!("WEBVTT\n\n{}", cues.join("\n"))
}

/// ASS style knobs that callers set (font family must match a file in `fontsdir`).
pub struct AssStyle<'a> {
    pub font: &'a str,
    pub size: u32,
    pub play_res: (u32, u32),
    /// Libass alignment, numpad layout: 2 = bottom centre.
    pub alignment: u8,
    pub margin_v: u32,
    /// Each page pops in (85 to 100 percent in 0.12 s): the short-form "pop" captions.
    pub pop: bool,
    /// Each word appears when it is said, in the accent colour, then turns white. No box: the box would show the
    /// page's width before its words.
    pub reveal: bool,
    /// Word indexes that stay in the accent colour for the whole page (brief `emphasis`).
    pub emphasis: Vec<usize>,
    /// Word indexes said by a second speaker (scene `voice`): they light up in `SECOND_SPEAKER`.
    pub second_speaker: Vec<usize>,
}

/// The spoken-word colour of a second speaker, light blue in ASS order: it reads on the caption box beside the accent.
pub const SECOND_SPEAKER: &str = "&H00FFD27F";

/// The accent colour in ASS order (&HAABBGGRR): the spoken-word highlight and the bars. A brief `accent` replaces it.
pub const ACCENT: &str = "&H0000E5FF";

/// `#RRGGBB` as an ASS colour, or None.
pub fn ass_colour(hex: &str) -> Option<String> {
    let h = hex
        .strip_prefix('#')
        .filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()))?;
    Some(format!("&H00{}{}{}", &h[4..6], &h[2..4], &h[0..2]).to_uppercase())
}

/// WCAG contrast of `#RRGGBB` against black (the caption box): 1 to 21. Under 3 the highlight cannot be read.
pub fn contrast_on_black(hex: &str) -> Option<f64> {
    let h = hex.strip_prefix('#').filter(|h| h.len() == 6)?;
    let ch = |i: usize| -> Option<f64> {
        let c = u8::from_str_radix(&h[i..i + 2], 16).ok()? as f64 / 255.0;
        Some(if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        })
    };
    let l = 0.2126 * ch(0)? + 0.7152 * ch(2)? + 0.0722 * ch(4)?;
    Some((l + 0.05) / 0.05)
}

/// Word indexes of each `emphasis` entry (a word or a phrase), everywhere it is said, and the entries not found. Case and
/// punctuation do not count, and Arabic letter forms compare as in the words gate.
pub fn find_emphasis(words: &[Word], wanted: &[String]) -> (Vec<usize>, Vec<String>) {
    let said: Vec<String> = words
        .iter()
        .map(|w| crate::gates::norm_word(&w.text))
        .collect();
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for entry in wanted {
        let want: Vec<String> = entry
            .split_whitespace()
            .map(crate::gates::norm_word)
            .collect();
        let hits: Vec<usize> = (0..said.len())
            .filter(|&i| !want.is_empty() && said[i..].starts_with(&want))
            .collect();
        if hits.is_empty() {
            missing.push(entry.clone());
        }
        for i in hits {
            found.extend(i..i + want.len());
        }
    }
    found.sort_unstable();
    found.dedup();
    (found, missing)
}

/// ASS with `\k` karaoke: each word lights up for its spoken time.
pub fn to_ass(pages: &[Page], words: &[Word], st: &AssStyle) -> String {
    let rtl = rtl_script(words);
    // Arabic inside a line of a font without Arabic: libass's own fallback draws it tiny and unjoined.
    let tag_arabic = crate::template::font_bytes(st.font).is_some_and(|b| !covers(b, '\u{0627}'));
    let mut out = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {}\nPlayResY: {}\nWrapStyle: 2\nScaledBorderAndShadow: yes\n\n\
[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Cap,{},{},{ACCENT},&H00FFFFFF,{},&H59000000,1,0,0,0,100,100,0,0,{},{},0,{},60,60,{},-1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        // Encoding -1: libass finds each line's direction (any other value forces left to right, so an Arabic
        // line that starts with a number came out reversed). BorderStyle 4 (libass): one 65% black box behind each line, so the text reads on any
        // footage. Style 3 draws one box per \\k word and the overlaps show as dark stripes.
        st.play_res.0,
        st.play_res.1,
        st.font,
        st.size,
        if st.reveal { "&H00000000" } else { "&H59000000" },
        if st.reveal { 1 } else { 4 },
        if st.reveal { st.size / 10 } else { st.size / 6 },
        st.alignment,
        st.margin_v
    );
    for p in pages {
        let mut cursor = p.start;
        let lines: Vec<String> = p
            .lines
            .iter()
            .map(|l| {
                let plain: String = l.iter().map(|&i| words[i].text.as_str()).collect();
                let line = l
                    .iter()
                    .map(|&i| {
                        let w = &words[i];
                        let strong = st.emphasis.contains(&i);
                        // In a dialogue every word names its colour: a tag lasts to the end of the line.
                        let colour = if st.second_speaker.contains(&i) {
                            SECOND_SPEAKER
                        } else {
                            ACCENT
                        };
                        let who = if st.second_speaker.is_empty() {
                            String::new()
                        } else {
                            format!("\\1c{colour}")
                        };
                        if st.reveal {
                            let ms = |t: f64| ((t - p.start) * 1000.0).round().max(0.0) as i64;
                            let (a, b) = (ms(w.start), ms(w.end));
                            if strong {
                                return format!(
                                    "{{\\alpha&HFF&\\1c{colour}\\t({a},{},\\alpha&H00&)}}{}",
                                    a + 60,
                                    ass_escape(&w.text)
                                );
                            }
                            return format!(
                                "{{\\alpha&HFF&\\1c{colour}\\t({a},{},\\alpha&H00&)\\t({b},{},\\1c&HFFFFFF&)}}{}",
                                a + 60,
                                b + 80,
                                ass_escape(&w.text)
                            );
                        }
                        // The wait before a word belongs to that word, so highlights never drift.
                        let cs = ((w.end - cursor) * 100.0).round().max(0.0) as u64;
                        cursor = w.end;
                        // Karaoke paints each word in the accent once said, so colour alone is lost by the end of the page:
                        // the word is also a size up (a 12 percent step fits inside the line margins).
                        if strong {
                            return format!(
                                "{{\\k{cs}{who}\\2c{colour}\\fscx112\\fscy112}}{}{{\\2c&HFFFFFF&\\fscx100\\fscy100}}",
                                ass_escape(&w.text)
                            );
                        }
                        format!("{{\\k{cs}{who}}}{}", ass_escape(&w.text))
                    })
                    .zip(l)
                    .map(|(s, &i)| {
                        if tag_arabic && words[i].text.chars().any(is_arabic) {
                            format!("{{\\fn{ARABIC_FAMILY}}}{s}{{\\fn{}}}", st.font)
                        } else {
                            s
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{}{line}", page_mark(&plain, rtl))
            })
            .collect();
        out.push_str(&format!(
            "Dialogue: 0,{},{},Cap,,0,0,0,,{}{}\n",
            clock(p.start, '.', true),
            clock(p.end, '.', true),
            if st.pop {
                "{\\fscx85\\fscy85\\t(0,120,\\fscx100\\fscy100)}"
            } else {
                ""
            },
            lines.join("\\N")
        ));
    }
    out
}

/// Brief `caption_size` as a fraction of the size the frame gives: `small` 4/5, `normal` (or none) 1/1, `large` 5/4,
/// or a percent (`"115%"` is 115/100).
/// Whole fractions, so `normal` is exactly the old size.
pub fn size_scale(name: Option<&str>) -> Result<(u32, u32), String> {
    match name {
        None | Some("normal") => Ok((1, 1)),
        Some("small") => Ok((4, 5)),
        Some("large") => Ok((5, 4)),
        // A free size: a percent of normal.
        Some(x) => x
            .strip_suffix('%')
            .and_then(|n| n.trim().parse::<u32>().ok())
            .filter(|n| (50..=200).contains(n))
            .map(|n| (n, 100))
            .ok_or(format!(
                "caption_size is small, normal or large, or a percent from \"50%\" to \"200%\", not {x}"
            )),
    }
}

/// Breaks a headline into lines no wider than `max_width` (measured, greedy).
pub fn wrap(text: &str, max_width: f32, width: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for w in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if width(&format!("{l} {w}")) <= max_width => {
                l.push(' ');
                l.push_str(w);
            }
            _ => lines.push(w.to_string()),
        }
    }
    lines
}

/// Adds a hook title card to an ASS document: a big centred headline in a box for
/// the first `secs` seconds, with a short pop-in (the usual short-form opening).
pub fn with_hook(ass: &str, lines: &[String], font: &str, size: u32, secs: f64) -> String {
    let style = format!(
        "Style: Hook,{font},{size},&H00FFFFFF,&H00FFFFFF,&H59000000,&H59000000,1,0,0,0,100,100,0,0,4,{},0,5,80,80,0,-1",
        size / 5
    );
    let line = format!(
        "Dialogue: 1,{},{},Hook,,0,0,0,,{{\\fad(80,200)\\fscx80\\fscy80\\t(0,180,\\fscx100\\fscy100)}}{}",
        clock(0.0, '.', true),
        clock(secs, '.', true),
        lines.iter().map(|l| format!("{}{}", rtl_mark(l), ass_escape(l))).collect::<Vec<_>>().join("\\N")
    );
    let mut out = String::with_capacity(ass.len() + 256);
    for l in ass.lines() {
        out.push_str(l);
        out.push('\n');
        if l.starts_with("Style: Cap,") {
            out.push_str(&style);
            out.push('\n');
        }
        if l.starts_with("Format: Layer,") {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// A small line in the top left corner for the whole video: a visible note such as "Made with AI".
pub fn with_label(ass: &str, label: &str, font: &str, size: u32) -> String {
    let style = format!(
        "Style: Label,{font},{size},&H00FFFFFF,&H00FFFFFF,&H59000000,&H59000000,0,0,0,0,100,100,0,0,4,{},0,7,40,40,40,-1",
        (size / 6).max(1)
    );
    let line = format!(
        "Dialogue: 2,{},9:59:59.00,Label,,0,0,0,,{}{}",
        clock(0.0, '.', true),
        rtl_mark(label),
        ass_escape(label)
    );
    let mut out = String::with_capacity(ass.len() + 256);
    for l in ass.lines() {
        out.push_str(l);
        out.push('\n');
        if l.starts_with("Style: Cap,") {
            out.push_str(&style);
            out.push('\n');
        }
        if l.starts_with("Format: Layer,") {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// Word index ranges of each sentence: a sentence ends at a word with an end mark, and words after the last mark are
/// one more sentence.
pub fn sentences<S: AsRef<str>>(words: &[S]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    for (i, w) in words.iter().enumerate() {
        if ends_sentence(w.as_ref()) {
            out.push(from..i + 1);
            from = i + 1;
        }
    }
    if from < words.len() {
        out.push(from..words.len());
    }
    out
}

/// The second caption line (brief `captions_second`): each cue `(start, end, lines)` in a smaller box at the bottom,
/// at `margin_v`, under the main captions, which the caller has raised to make room.
pub fn with_second(
    ass: &str,
    cues: &[(f64, f64, Vec<String>)],
    font: &str,
    size: u32,
    margin_v: u32,
) -> String {
    let style = format!(
        "Style: Cap2,{font},{size},&H00E6E6E6,&H00E6E6E6,&H59000000,&H59000000,0,0,0,0,100,100,0,0,4,{},0,2,60,60,{margin_v},-1",
        (size / 6).max(1)
    );
    let events: String = cues
        .iter()
        .map(|(a, b, lines)| {
            format!(
                "Dialogue: 0,{},{},Cap2,,0,0,0,,{}\n",
                clock(*a, '.', true),
                clock(*b, '.', true),
                lines
                    .iter()
                    .map(|l| format!("{}{}", rtl_mark(l), ass_escape(l)))
                    .collect::<Vec<_>>()
                    .join("\\N")
            )
        })
        .collect();
    let mut out = String::with_capacity(ass.len() + events.len() + 256);
    for l in ass.lines() {
        out.push_str(l);
        out.push('\n');
        if l.starts_with("Style: Cap,") {
            out.push_str(&style);
            out.push('\n');
        }
        if l.starts_with("Format: Layer,") {
            out.push_str(&events);
        }
    }
    out
}

/// Text from a script must not open ASS override blocks or line breaks.
fn ass_escape(s: &str) -> String {
    s.replace('\\', "\u{29F5}")
        .replace('{', "(")
        .replace('}', ")")
}

#[cfg(test)]
mod tests {
    /// T8, 13 Nov 2026: the result names the caption files beside the video, not only those in the run folder.
    #[test]
    fn the_result_names_the_captions_beside_the_video() {
        let d = tempfile::tempdir().unwrap();
        let cue = "00:00:00.000 --> 00:00:01.000\nHi\n";
        std::fs::write(d.path().join("captions.srt"), format!("1\n{cue}")).unwrap();
        std::fs::write(d.path().join("captions.vtt"), format!("WEBVTT\n\n{cue}")).unwrap();
        let video = d.path().join("name.1.mp4");
        let mut env = crate::envelope::Envelope::ok("make");
        super::sidecars(d.path(), &video, true, &mut env);
        for ext in ["srt", "vtt"] {
            let at = env.artifacts[&format!("sidecar_{ext}")].as_str().unwrap();
            assert!(at.ends_with(&format!("name.1.{ext}")), "{at}");
            assert!(std::path::Path::new(at).is_file(), "{at}");
        }
    }

    /// A video with no words: its caption files have no cue, and none is copied beside the video.
    #[test]
    fn a_caption_file_with_no_cue_is_not_copied_beside_the_video() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("captions.srt"), "").unwrap();
        std::fs::write(d.path().join("captions.vtt"), "WEBVTT\n\n").unwrap();
        let mut env = crate::envelope::Envelope::ok("make");
        super::sidecars(d.path(), &d.path().join("v.mp4"), true, &mut env);
        assert!(env.artifacts.get("sidecar_srt").is_none() && !d.path().join("v.srt").exists());
        assert!(env.artifacts.get("sidecar_vtt").is_none() && !d.path().join("v.vtt").exists());
    }

    /// make-7: credits beside `--out` follow the rule of the subtitles: a user's file stays.
    #[test]
    fn a_credits_file_the_user_wrote_is_kept() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("credits.txt"), "new credits").unwrap();
        let video = d.path().join("v.mp4");
        let mine = d.path().join("v.credits.txt");
        std::fs::write(&mine, "my credits").unwrap();
        let mut env = crate::envelope::Envelope::ok("make");
        super::sidecars(d.path(), &video, true, &mut env);
        assert_eq!(std::fs::read_to_string(&mine).unwrap(), "my credits");
        let new = env.artifacts["credits"].as_str().unwrap();
        assert!(new.ends_with("v-2.credits.txt"), "{new}");
    }

    /// make-9: an English page with Arabic words stays left to right and draws them with the Arabic font.
    #[test]
    fn arabic_words_in_an_english_script_keep_the_page_direction_and_get_their_font() {
        let words: Vec<Word> = ["We", "visited", "\u{0645}\u{0635}\u{0631}", "last"]
            .iter()
            .enumerate()
            .map(|(i, t)| w(t, i as f64, i as f64 + 1.0))
            .collect();
        let pages = vec![Page {
            lines: vec![(0..4).collect()],
            start: 0.0,
            end: 4.0,
            said_end: 4.0,
        }];
        let st = AssStyle {
            font: "Noto Sans",
            size: 72,
            play_res: (1080, 1920),
            alignment: 2,
            margin_v: 260,
            pop: false,
            reveal: false,
            emphasis: vec![],
            second_speaker: vec![],
        };
        let ass = to_ass(&pages, &words, &st);
        assert!(ass.contains("Cap,,0,0,0,,\u{200E}{"), "{ass}");
        assert!(!ass.contains('\u{200F}'), "{ass}");
        assert!(ass.contains("{\\fnNoto Sans Arabic}{\\k"), "{ass}");
        assert!(
            ass.contains("{\\fnNoto Sans} last") || ass.contains("{\\fnNoto Sans} {\\k"),
            "{ass}"
        );
        assert!(to_srt(&pages, &words).contains("\u{200E}We"));
        // The whole script is Arabic: the page stays right to left even when the line opens in Latin.
        let ar: Vec<Word> = [
            "Apple",
            "\u{0645}\u{0635}\u{0631}",
            "\u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064A}\u{0629}",
        ]
        .iter()
        .map(|t| w(t, 0.0, 1.0))
        .collect();
        let page = Page {
            lines: vec![vec![0, 1, 2]],
            start: 0.0,
            end: 1.0,
            said_end: 1.0,
        };
        assert!(to_srt(&[page], &ar).contains("\u{200F}Apple"));
    }

    /// later-2: a line in two scripts is measured with both fonts, as it is drawn.
    #[test]
    fn a_mixed_line_is_measured_with_each_scripts_font() {
        let font = |f: &str| crate::template::font_bytes(f).unwrap().to_vec();
        let latin = Measurer::new(font("Noto Sans"), 72.0).unwrap();
        let arabic = Measurer::new(font("Noto Sans Arabic"), 72.0).unwrap();
        let word = "\u{0645}\u{0635}\u{0631}";
        let got = latin.width(&format!("Hello {word}"));
        let want = latin.width("Hello ") + arabic.width(word);
        assert!((got - want).abs() < 0.01, "{got} {want}");
    }

    #[test]
    fn label_adds_one_style_and_one_line() {
        let ass = "[V4+ Styles]\nStyle: Cap,F,10\n[Events]\nFormat: Layer, Start\n";
        let out = super::with_label(ass, "Made {with} AI", "F", 20);
        assert_eq!(out.matches("Style: Label,F,20").count(), 1);
        assert_eq!(out.matches("Dialogue: 2,").count(), 1);
        assert!(!out.contains("{with}"), "{out}");
    }

    use super::*;

    fn w(text: &str, start: f64, end: f64) -> Word {
        Word {
            text: text.into(),
            start,
            end,
        }
    }

    // ponytail: 10 px per char stands in for shaping in unit tests; tests/captions.rs uses the real font.
    fn chars(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    #[test]
    fn second_line_goes_under_each_sentence() {
        let w = ["Rain", "falls.", "Drains", "work!", "Then", "more"];
        assert_eq!(sentences(&w), vec![0..2, 2..4, 4..6]);
        assert_eq!(sentences(&["يسقط", "المطر؟"]), vec![0..2]);
        let ass = "[V4+ Styles]\nStyle: Cap,X\n[Events]\nFormat: Layer, Start\nDialogue: 0,main\n";
        let out = with_second(
            ass,
            &[(0.0, 1.5, vec!["un".into(), "deux".into()])],
            "Noto Sans",
            50,
            300,
        );
        assert!(out.contains("Style: Cap2,Noto Sans,50,"), "{out}");
        assert!(out.contains(",2,60,60,300,-1"), "{out}");
        assert!(
            out.contains("Dialogue: 0,0:00:00.00,0:00:01.50,Cap2,,0,0,0,,un\\Ndeux\n"),
            "{out}"
        );
    }

    #[test]
    fn vtt_has_the_srt_cues_with_dots() {
        let words = vec![
            w("one", 0.0, 0.3),
            w("two.", 0.3, 0.6),
            w("three", 61.25, 61.5),
        ];
        let l = Layout {
            max_width: 80.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 99.0,
        };
        let pages = paginate(&words, &l, chars);
        let vtt = to_vtt(&pages, &words);
        let srt = to_srt(&pages, &words);
        assert!(vtt.starts_with("WEBVTT\n\n00:00:00.000 --> "), "{vtt}");
        assert!(vtt.contains("00:01:01.250"), "{vtt}");
        assert_eq!(vtt.matches(" --> ").count(), srt.matches(" --> ").count());
    }

    /// 13 Nov 2026, two scenes joined: the words of scene 1 ran to 3.825 s (the silence at the end of its clip, which
    /// the join cuts), scene 2 started at 3.479 s. Whatever the word times are, one page is on screen at a time.
    #[test]
    fn a_page_ends_before_the_next_one_starts_when_word_times_overlap() {
        let scene = |text: &str, from: f64, to: f64| -> Vec<Word> {
            let t: Vec<&str> = text.split_whitespace().collect();
            let step = (to - from) / t.len() as f64;
            t.iter()
                .enumerate()
                .map(|(i, x)| w(x, from + i as f64 * step, from + (i + 1) as f64 * step))
                .collect()
        };
        let mut words = scene("This year was a good year for us.", 0.0, 3.825);
        words.extend(scene(
            "Revenue grew by forty two percent in one year.",
            3.479,
            8.379,
        ));
        let l = Layout {
            max_width: 40.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 4.0,
        };
        let pages = paginate(&words, &l, chars);
        assert!(pages.len() >= 2);
        for p in pages.windows(2) {
            assert!(p[0].end <= p[1].start, "{} > {}", p[0].end, p[1].start);
            assert!(p[0].start <= p[0].said_end && p[0].said_end <= p[0].end);
        }
        // The last page of scene 1 had its end at 3.825 s; it now ends where scene 2 starts.
        let k = pages.iter().position(|p| p.start == 3.479).unwrap();
        assert_eq!(pages[k - 1].end, 3.479);
        assert_eq!(crate::gates::caption_pages(&pages).status, "pass");
    }

    /// T3, 13 Nov 2026: "W. H. Auden" gave a page that held "H." alone.
    #[test]
    fn an_initial_or_a_short_form_does_not_end_a_page() {
        let said = |text: &str| -> Vec<Word> {
            text.split_whitespace()
                .enumerate()
                .map(|(i, t)| w(t, i as f64 * 0.3, i as f64 * 0.3 + 0.25))
                .collect()
        };
        let l = Layout {
            max_width: 300.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 99.0,
        };
        let pages_of = |text: &str| -> Vec<String> {
            let words = said(text);
            paginate(&words, &l, chars)
                .iter()
                .map(|p| {
                    p.lines
                        .iter()
                        .flatten()
                        .map(|&i| words[i].text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect()
        };
        assert_eq!(
            pages_of(
                "The poet W. H. Auden wrote it. Dr. Lee read it in the U.S. Senate, e.g. twice."
            ),
            [
                "The poet W. H. Auden wrote it.",
                "Dr. Lee read it in the U.S. Senate, e.g. twice."
            ]
        );
        for end in [
            "it.",
            "it.\"",
            "end!",
            "why?",
            "I.",
            "No.",
            "3.5.",
            "المطر.",
            "المطر؟",
            "ا.",
            "د.",
        ] {
            assert!(ends_sentence(end), "{end}");
        }
        for not in ["W.", "(H.", "U.S.", "e.g.", "a.m.", "Dr.", "Mrs.", "vs."] {
            assert!(!ends_sentence(not), "{not}");
        }
        // Arabic pages are cut at the same sentence ends as before.
        assert_eq!(
            pages_of("يسقط المطر. ثم يتوقف؟ نعم."),
            ["يسقط المطر.", "ثم يتوقف؟", "نعم."]
        );
    }

    #[test]
    fn breaks_by_width_and_sentence() {
        let words = vec![
            w("one", 0.0, 0.3),
            w("two", 0.3, 0.6),
            w("three.", 0.6, 1.0),
            w("four", 1.1, 1.4),
        ];
        let l = Layout {
            max_width: 80.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 99.0,
        };
        let pages = paginate(&words, &l, chars);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].lines, vec![vec![0, 1], vec![2]]);
        assert_eq!(pages[1].lines, vec![vec![3]]);
    }

    #[test]
    fn last_word_of_a_sentence_is_never_alone_on_a_page() {
        let words = vec![
            w("aa", 0.0, 0.1),
            w("bb", 0.1, 0.2),
            w("cc", 0.2, 0.3),
            w("dd", 0.3, 0.4),
            w("ee.", 0.4, 0.5),
        ];
        let l = Layout {
            max_width: 50.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 99.0,
        };
        let pages = paginate(&words, &l, chars);
        assert_eq!(pages.last().unwrap().lines, vec![vec![3, 4]]);
    }

    #[test]
    fn a_line_does_not_break_before_a_variable() {
        // Without the rule: "area equals pi r" / "squared now.".
        let t = ["area", "equals", "pi", "r", "squared", "now."];
        let words: Vec<_> = t
            .iter()
            .enumerate()
            .map(|(i, x)| w(x, i as f64 * 0.2, i as f64 * 0.2 + 0.2))
            .collect();
        for max_lines in [1, 2] {
            let l = Layout {
                max_width: 160.0,
                max_lines,
                max_gap: 0.6,
                max_page: 99.0,
            };
            let pages = paginate(&words, &l, chars);
            let lines: Vec<Vec<&str>> = pages
                .iter()
                .flat_map(|p| p.lines.iter())
                .map(|l| l.iter().map(|&i| t[i]).collect())
                .collect();
            assert_eq!(lines[0], ["area", "equals"], "{max_lines}: {lines:?}");
            assert!(lines[1].len() >= 3, "{max_lines}: {lines:?}");
            assert_eq!(
                lines[1][..3],
                ["pi", "r", "squared"],
                "{max_lines}: {lines:?}"
            );
        }
        assert!(leans_back("r") && leans_back("x.") && leans_back("squared"));
        assert!(!leans_back("a") && !leans_back("I") && !leans_back("pi") && !leans_back("5"));
    }

    #[test]
    fn a_page_does_not_end_on_a_weak_word() {
        // Width fits 3 short words a line: without the rule page 1 would end "...dug to".
        let t = ["we", "dug", "to", "aa", "bb", "cc", "dd."];
        let words: Vec<_> = t
            .iter()
            .enumerate()
            .map(|(i, x)| w(x, i as f64 * 0.2, i as f64 * 0.2 + 0.2))
            .collect();
        let l = Layout {
            max_width: 90.0,
            max_lines: 1,
            max_gap: 0.6,
            max_page: 99.0,
        };
        let pages = paginate(&words, &l, chars);
        let first: Vec<_> = pages[0].lines[0].iter().map(|&i| t[i]).collect();
        assert_eq!(first, ["we", "dug"], "{pages:?}");
        assert_eq!(pages[1].lines[0][0], 2, "\"to\" opens page 2");
        assert!(weak("في") && weak("The") && !weak("to,") && !weak("channels"));
        assert!(weak("≈") && weak("→") && !weak("2×"));
    }

    #[test]
    fn a_page_that_ends_by_time_hands_on_its_weak_word() {
        let t = [
            "لذلك",
            "حفر",
            "المزارعون",
            "تحمل",
            "الماء",
            "إلى",
            "النخيل.",
        ];
        let words: Vec<_> = t
            .iter()
            .enumerate()
            .map(|(i, x)| w(x, i as f64 * 0.6, i as f64 * 0.6 + 0.55))
            .collect();
        let l = Layout {
            max_width: 400.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 3.5,
        };
        let pages = paginate(&words, &l, chars);
        let ends: Vec<_> = pages
            .iter()
            .map(|p| t[*p.lines.last().unwrap().last().unwrap()])
            .collect();
        assert!(!ends.contains(&"إلى"), "{pages:?}");
        // The case from a real render: "إلى" alone on line 2 when the page ends by time.
        let narrow = Layout {
            max_width: 220.0,
            ..l
        };
        let pages = paginate(&words, &narrow, chars);
        let ends: Vec<_> = pages
            .iter()
            .map(|p| t[*p.lines.last().unwrap().last().unwrap()])
            .collect();
        assert!(!ends.contains(&"إلى"), "{pages:?}");
    }

    #[test]
    fn a_page_that_ends_by_time_keeps_a_sentence_end_company() {
        let t = ["aa", "bb", "cc", "dd", "ee."];
        let words: Vec<_> = t
            .iter()
            .enumerate()
            .map(|(i, x)| w(x, i as f64, i as f64 + 0.9))
            .collect();
        let l = Layout {
            max_width: 999.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 4.0,
        };
        let pages = paginate(&words, &l, chars);
        // "ee." never alone: it keeps company (the short-tail rule may give it two words).
        assert!(pages.last().unwrap().lines.concat().len() >= 2, "{pages:?}");
    }

    #[test]
    fn a_number_keeps_its_thousand_on_the_same_page() {
        let t = ["بسعر", "نحو", "أربعة", "آلاف", "درهم."];
        let words: Vec<_> = t
            .iter()
            .enumerate()
            .map(|(i, x)| w(x, i as f64, i as f64 + 0.9))
            .collect();
        let l = Layout {
            max_width: 999.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 3.5,
        };
        let pages = paginate(&words, &l, chars);
        let starts: Vec<_> = pages.iter().map(|p| t[p.lines[0][0]]).collect();
        assert!(!starts.contains(&"آلاف"), "{pages:?}");
        // The real render: the time limit falls on "درهم." and its orphan carry must not split "أربعة آلاف".
        let words: Vec<_> = t
            .iter()
            .enumerate()
            .map(|(i, x)| w(x, i as f64, i as f64 + 0.9))
            .collect();
        let l = Layout { max_page: 4.5, ..l };
        let pages = paginate(&words, &l, chars);
        let starts: Vec<_> = pages.iter().map(|p| t[p.lines[0][0]]).collect();
        assert!(!starts.contains(&"آلاف"), "{pages:?}");
        let ends: Vec<_> = pages
            .iter()
            .map(|p| t[*p.lines.last().unwrap().last().unwrap()])
            .collect();
        assert!(
            !ends.contains(&"نحو"),
            "a page does not end on 'about' either: {pages:?}"
        );
        assert!(number_tail("thousand,") && number_tail("آلاف") && !number_tail("four"));
    }

    /// French sets "?", "!", ":" and the quote marks off with a space. No line or page starts with a closing mark or
    /// ends with an opening one, at any line width.
    #[test]
    fn a_mark_with_a_space_stays_with_its_word() {
        let text = "Il dit : « bonjour » à tous ! Vraiment ? Oui ; « le marché ouvre » et la place est calme !";
        let words: Vec<Word> = text
            .split_whitespace()
            .enumerate()
            .map(|(i, t)| w(t, i as f64 * 0.3, i as f64 * 0.3 + 0.25))
            .collect();
        for max_width in [8.0, 10.0, 12.0, 14.0, 17.0, 20.0, 26.0] {
            for max_page in [1.5, 2.5, 99.0] {
                let l = Layout {
                    max_width,
                    max_lines: 2,
                    max_gap: 0.6,
                    max_page,
                };
                for p in paginate(&words, &l, chars) {
                    for line in &p.lines {
                        let (first, last) =
                            (&words[line[0]].text, &words[*line.last().unwrap()].text);
                        assert!(
                            !closes(first),
                            "width {max_width}, page {max_page}: a line starts with {first}"
                        );
                        assert!(
                            !opens(last),
                            "width {max_width}, page {max_page}: a line ends with {last}"
                        );
                    }
                }
            }
        }
    }

    /// The bundled caption font draws the letters and marks of the five Latin languages (docs/specs/latin-languages.md).
    #[test]
    fn the_caption_font_has_the_letters_of_the_latin_languages() {
        let font = crate::template::font_bytes("Noto Sans").unwrap();
        for c in "ñÑáéíóúüÁÉÍÓÚÜ¿¡àâæçèêëîïôœùûÿÀÂÆÇÈÊËÎÏÔŒÙÛŸ«»äöüßÄÖẞãõÃÕ€‹›„“”’–—…\u{a0}\u{202f}"
            .chars()
        {
            assert!(covers(font, c), "Noto Sans has no glyph for {c:?}");
        }
    }

    /// The page rules of the new languages: no page ends on "de" or "und", and a name in capitals is not such a word.
    #[test]
    fn weak_words_of_the_latin_languages() {
        // Not in an English or Arabic script: these words are names there ("Dan", "de facto").
        for t in ["de", "les", "und", "não", "yang", "dan"] {
            assert!(!weak(t), "{t}");
        }
        assert!(!number_tail("mil"));
        LATIN.set(true);
        for t in ["de", "les", "und", "não", "yang"] {
            assert!(weak(t), "{t}");
        }
        for t in ["MIT", "EST", "die", "son", "casa", "de."] {
            assert!(!weak(t), "{t}");
        }
        assert!(number_tail("millones") && number_tail("ribu,"));
        assert!(!number_tail("cent"));
        LATIN.set(false);
    }

    #[test]
    fn arabic_lines_get_a_right_to_left_mark() {
        assert_eq!(rtl_mark("2026 بسعر"), "\u{200F}");
        assert_eq!(rtl_mark("Apple"), "");
        assert_eq!(rtl_mark("in 2026"), "");
    }

    #[test]
    fn a_sentence_end_does_not_stand_alone_on_a_page() {
        let ws: Vec<Word> =
            "The asphalt soaks up the sun all day and gives the heat back all night."
                .split(' ')
                .enumerate()
                .map(|(i, t)| Word {
                    text: t.into(),
                    start: i as f64 * 0.3,
                    end: i as f64 * 0.3 + 0.25,
                })
                .collect();
        let layout = Layout {
            max_width: 30.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 99.0,
        };
        let pages = paginate(&ws, &layout, |t| t.chars().count() as f32);
        let last: usize = pages.last().unwrap().lines.iter().map(Vec::len).sum();
        assert!(
            last >= 3,
            "{:?}",
            pages.iter().map(|p| p.lines.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_page_holds_through_a_short_pause_only() {
        let words = vec![
            w("One.", 0.0, 0.5),
            w("Two.", 0.9, 1.4),
            w("Three.", 3.0, 3.5),
        ];
        let l = Layout {
            max_width: 500.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 99.0,
        };
        let p = paginate(&words, &l, chars);
        assert_eq!(p.len(), 3);
        assert!(
            (p[0].end - p[1].start).abs() < 1e-9,
            "a 0.4 s pause is held"
        );
        assert!(
            p[2].start - p[1].end > 1.0,
            "a 1.6 s pause still clears the screen"
        );
    }

    #[test]
    fn long_gap_starts_a_page() {
        let words = vec![w("a", 0.0, 0.2), w("b", 2.0, 2.2)];
        let l = Layout {
            max_width: 500.0,
            max_lines: 2,
            max_gap: 0.6,
            max_page: 99.0,
        };
        assert_eq!(paginate(&words, &l, chars).len(), 2);
    }

    #[test]
    fn script_text_cannot_inject_ass_tags() {
        assert_eq!(ass_escape(r"{\b1}x\N"), "(\u{29F5}b1)x\u{29F5}N");
    }

    #[test]
    fn align_restores_punctuation_and_merges_split_words() {
        let ev = vec![
            w("Hello", 0.0, 0.4),
            w("twenty", 0.5, 0.8),
            w("twenty-six", 0.8, 1.2),
            w("now", 1.3, 1.5),
        ];
        let a = align("Hello, 2026 now.", &ev);
        let texts: Vec<_> = a.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, ["Hello,", "2026", "now."]);
        assert_eq!((a[1].start, a[1].end), (0.5, 1.2));
    }

    #[test]
    fn align_keeps_later_words_in_place_after_a_respelled_number() {
        // Whisper on a recording, 6 Nov 2026: "one point four" came as "1" ".4", and "and" drifted 2.9 s.
        let ev = vec![
            w("about", 1.56, 1.8),
            w("1", 1.8, 2.12),
            w(".4", 2.12, 2.64),
            w("million", 2.64, 2.88),
            w("new", 9.48, 9.78),
            w("um", 9.78, 10.0),
            w("parks", 10.92, 11.5),
        ];
        let a = align("about one point four million, new parks.", &ev);
        let t: Vec<_> = a.iter().map(|x| (x.text.as_str(), x.start)).collect();
        assert_eq!(t[4], ("million,", 2.64));
        assert_eq!(t[5], ("new", 9.48));
        assert_eq!(
            a[5].end, 10.0,
            "the extra event stretches the word before it"
        );
        assert_eq!(t[6], ("parks.", 10.92));
        assert!(t[1].1 == 1.8 && t[2].1 > 1.8 && t[3].1 < 2.64, "{t:?}");
    }

    #[test]
    fn align_splits_an_event_that_holds_two_words() {
        // Edge, 4 Nov 2026: "In 2005" came as one event and shifted every later word by one.
        let ev = vec![
            w("fast", 0.9, 1.5),
            w("In 2005", 1.95, 2.95),
            w("one", 2.96, 3.15),
            w("point", 3.16, 3.44),
            w("parks", 12.2, 12.7),
        ];
        let a = align("fast. In 2005 one point parks.", &ev);
        let got: Vec<_> = a.iter().map(|x| (x.text.as_str(), x.start)).collect();
        assert_eq!(got[2].0, "2005");
        assert!(
            (got[1].1 - 1.95).abs() < 1e-9 && (got[2].1 - (1.95 + 1.0 / 3.0)).abs() < 1e-9,
            "{got:?}"
        );
        assert_eq!((got[3].0, got[3].1), ("one", 2.96));
        assert_eq!((a[5].start, a[5].end), (12.2, 12.7));
    }

    #[test]
    fn hook_adds_a_style_and_a_first_line() {
        let st = AssStyle {
            font: "Noto Sans",
            size: 84,
            play_res: (1080, 1920),
            alignment: 2,
            margin_v: 360,
            pop: false,
            reveal: false,
            emphasis: vec![],
            second_speaker: vec![],
        };
        let base = to_ass(&[], &[], &st);
        let h = with_hook(
            &base,
            &["Stop {scrolling}".to_string()],
            "Noto Sans",
            130,
            1.6,
        );
        assert!(h.contains("Style: Hook,Noto Sans,130,"));
        assert!(h.contains("Dialogue: 1,0:00:00.00,0:00:01.60,Hook,"));
        assert!(
            h.contains("Stop (scrolling)"),
            "script text cannot open override blocks"
        );
    }

    #[test]
    fn a_flash_word_joins_a_neighbour_page() {
        let words = [
            w("one", 0.0, 0.5),
            w("of", 0.5, 0.6),
            w("three", 0.6, 1.2),
            w("words", 1.2, 1.7),
        ];
        let pop = Layout {
            max_width: 100.0,
            max_lines: 1,
            max_gap: 0.4,
            max_page: 0.5,
        };
        let pages = paginate(&words, &pop, chars);
        assert!(
            pages.iter().all(|p| p.end - p.start >= FLASH_S),
            "{pages:?}"
        );
        let n: usize = pages.iter().map(|p| p.lines.concat().len()).sum();
        assert_eq!(n, 4);
    }

    #[test]
    fn a_flash_word_with_no_room_on_either_side_takes_the_word_before() {
        let words = [
            w("Most", 0.1, 0.33),
            w("floods", 0.34, 0.65),
            w("start", 0.66, 0.95),
            w("with", 0.96, 1.07),
            w("a", 1.09, 1.14),
            w("blocked", 1.15, 1.49),
            w("drain.", 1.5, 1.85),
        ];
        let page = |lines: Vec<usize>, start: f64, end: f64| Page {
            lines: vec![lines],
            start,
            end,
            said_end: end,
        };
        let mut pages = vec![
            page(vec![0, 1, 2], 0.1, 0.96),
            page(vec![3], 0.96, 1.087),
            page(vec![4, 5, 6], 1.087, 1.85),
        ];
        let layout = Layout {
            max_width: 17.0,
            max_lines: 1,
            max_gap: 0.4,
            max_page: 3.0,
        };
        no_flash(&mut pages, &words, &layout, &|s: &str| s.len() as f32);
        assert_eq!(pages[0].lines, vec![vec![0, 1]]);
        assert_eq!(pages[1].lines, vec![vec![2, 3]]);
        assert!(pages[1].end - pages[1].start >= FLASH_S, "{pages:?}");
        assert!((pages[0].end - 0.66).abs() < 1e-9, "{pages:?}");
    }

    #[test]
    fn pop_pages_scale_in() {
        let words = [w("Hi", 0.0, 0.4), w("there", 0.4, 0.9)];
        let page = Page {
            lines: vec![vec![0, 1]],
            start: 0.0,
            end: 0.9,
            said_end: 0.9,
        };
        let mut st = AssStyle {
            font: "Noto Sans",
            size: 113,
            play_res: (1080, 1920),
            alignment: 2,
            margin_v: 360,
            pop: true,
            reveal: false,
            emphasis: vec![],
            second_speaker: vec![],
        };
        let ass = to_ass(std::slice::from_ref(&page), &words, &st);
        assert!(
            ass.contains(",Cap,,0,0,0,,{\\fscx85\\fscy85\\t(0,120,\\fscx100\\fscy100)}{\\k40}Hi"),
            "{ass}"
        );
        st.pop = false;
        assert!(!to_ass(std::slice::from_ref(&page), &words, &st).contains("fscx85"));
        // Reveal: the second word stays hidden until 400 ms into the page, and no box.
        st.reveal = true;
        let ass = to_ass(&[page], &words, &st);
        assert!(ass.contains("\\t(400,460,\\alpha&H00&)"), "{ass}");
        assert!(
            ass.contains("&H00000000,&H59000000,1,0,0,0,100,100,0,0,1,"),
            "{ass}"
        );
        // An emphasised word keeps its colour: no turn back to white in reveal, accent before it is said in karaoke.
        st.emphasis = vec![1];
        let page = Page {
            lines: vec![vec![0, 1]],
            start: 0.0,
            end: 0.9,
            said_end: 0.9,
        };
        let ass = to_ass(std::slice::from_ref(&page), &words, &st);
        assert_eq!(ass.matches("&HFFFFFF&)").count(), 1, "{ass}");
        st.reveal = false;
        let ass = to_ass(std::slice::from_ref(&page), &words, &st);
        assert!(ass.contains(&format!("\\2c{ACCENT}\\fscx112")), "{ass}");
        // A second speaker: each word names its colour, because a tag lasts to the end of the line.
        st.emphasis = vec![];
        st.second_speaker = vec![1];
        let ass = to_ass(std::slice::from_ref(&page), &words, &st);
        assert!(ass.contains(&format!("\\1c{ACCENT}}}Hi")), "{ass}");
        assert!(
            ass.contains(&format!("\\1c{SECOND_SPEAKER}}}there")),
            "{ass}"
        );
    }

    #[test]
    fn emphasis_finds_words_and_phrases_in_their_spoken_forms() {
        let words: Vec<Word> = ["The", "Tide,", "came", "in", "at", "90%,", "fast."]
            .iter()
            .map(|t| Word {
                text: t.to_string(),
                start: 0.0,
                end: 0.1,
            })
            .collect();
        let (found, missing) = find_emphasis(
            &words,
            &["tide".into(), "came in".into(), "90%".into(), "slow".into()],
        );
        assert_eq!(found, vec![1, 2, 3, 5]);
        assert_eq!(missing, vec!["slow".to_string()]);
    }

    #[test]
    fn wrap_keeps_lines_inside_the_width() {
        assert_eq!(
            wrap("one two three four", 90.0, chars),
            ["one two", "three", "four"]
        );
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(3723.456, ',', false), "01:02:03,456");
        assert_eq!(clock(3723.456, '.', true), "1:02:03.46");
    }
}
