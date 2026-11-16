# `scriptvox report`: a text to send with a problem (G126, 10 Nov 2026)

Why: a user who meets a fault should be able to hand over what happened without pasting keys or a home folder into an issue.

- `scriptvox report [run] [--out file]` writes one Markdown file and sends nothing. Default file: `scriptvox-out/report-<run>.md`, and the next free number when it exists. A named `--out` that exists is refused.
- The run: the id given, else the newest run of the project that failed or died, else the newest run. With no run at all the text has the machine part only. A run id may hold letters, digits, `_` and `.`; anything else is exit 2, so no other path can be read.
- In the text: the version, the system, the checks of `doctor --offline` (no network call), the names of the keys that are set, the 5 latest runs, and for the run its result (status, error, findings, metrics, next), its request, and the last 40 lines of `events.jsonl` and 20 of `supervisor.log`.
- Taken out: every key value (the same redactor as every output line), the home folder and the user name, and in the request the texts of `script`, `prompt`, `hook`, card `text`, `label` and `items`, `sources`, `lines` and the like: each becomes its length, such as `<25 characters>`.
- Not taken out: an error or a gate message can quote a few of the user's words, for example a title that left the frame. The text says so in its first lines and tells the user to read it first.

Run on 10 Nov 2026 at home on a real failed run (a title too wide, exit 3): 174 lines; a search of the file for the home folder, the user name, both key values and the script's own words found none. The gate message in it does quote the first 32 characters of the wide title, which is the limit stated above. Tests: `the_words_of_a_request_are_replaced_by_their_size` (unit) and `report_writes_a_local_text_without_keys_paths_or_the_script` (a key and the home folder planted in a log line, a second report, a named file that exists, and 3 bad ids).

Not done: not an MCP tool; no option to leave out the logs. Not run: Windows (there the home folder comes from another variable, so the path part may stay).
