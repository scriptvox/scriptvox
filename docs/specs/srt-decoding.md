# Caption files keep their words (section 6 row 13, E10)

Fault: `edit --captions-file` read a file with `from_utf8_lossy`, so Arabic in Windows-1256 became replacement characters with no error, and those were burned into the video. `export --soft-subs` handed the file to FFmpeg unread.

`edit::decode_srt`, used by both: a BOM (UTF-8 or UTF-16 LE/BE); else UTF-8; else Windows-1256 when at least 70 percent of the high bytes read as Arabic. Anything else stops with a named cause and the file name: the file is not UTF-8 and its bytes do not read as Arabic; or the file holds U+FFFD, so an earlier conversion damaged it. FFmpeg gets a UTF-8 copy for `--soft-subs`.

Reused from filmcraft `crates/captions/src/srt.rs` at 72831a4: nothing copied; it was read, and its fallback (Windows-1252) is the wrong one for us. The table is Windows-1256 from the published code page.

Test: `edit::tests::caption_bytes_decode_or_stop_with_a_cause`. Real run, Arabic captions from a real video: Windows-1256 and UTF-16 copies burned correctly with `--captions-file` (read on a 4-frame sheet); a Latin-1 file stopped with exit 2 and the cause. Not run: `--soft-subs` with a Windows-1256 file (it uses the same function).
