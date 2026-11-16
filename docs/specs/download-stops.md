# A retry that cannot help stops at once (section 6 row 11, E8 and G078)

A removed file (404, 410), a refused host (401, 403 on any host; the gated Hugging Face case keeps its own message), a full disk or a failed TLS check were reported as a network problem and retried. `provision::stop_cause` stops these at once, with the URL or file and a named next step (`STOP`, handled in `doctor::setup_failure`). A dropped connection still retries.

Downloads (9 Nov 2026): one `install` per file at a time, by a lock file in `downloads/`, so a second setup waits and then finds the file. A read that gets no byte for 60 s counts as a break (`SCRIPTVOX_DOWNLOAD_IDLE_SECS`). A break or a stall keeps the `.part`, and the next try and the next run ask for the rest with `Range`; 3 tries in a row with no new byte end it. The sha256 is taken from the finished file. A bad kept part is deleted and fetched once more. Only a connection error is retryable (exit 5, `provision::network_fault`): a folder, a disk or an unpack on this machine is exit 1 and names the data folder. A failed automatic FFmpeg download in any command answers the same way (`engine_ffmpeg::locate_failure`).

`just pins-check` HEADs every pin in `toolchain.lock`, so a removed file is seen before a user meets it. It uses the network and is not part of `just ci`; the owner's timer can run it.

Test: `a_missing_file_stops_at_once_with_the_url` (one hit, no retry). Real run: `setup` on a 404 URL at Hugging Face stopped with the cause; the pin check ran. Not run: a real full disk, a real TLS failure.
