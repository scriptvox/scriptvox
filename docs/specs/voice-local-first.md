# An installed local voice comes first

`voices::choose` picks, among the voices that fit the language, engine and id and can run now, the first one with
`network = false`; when none is local, the first in registry order (Edge). So an installed Kokoro voice is the
English default, and the script does not go to a network service.

Not in this change: a free local Arabic voice. Arabic has only network or selfhost rows, so Edge stays the Arabic
default. A CPU Arabic voice through `setup` is an open plan item.

Test: `voices::tests::a_local_voice_wins_over_edge_when_both_can_run`. Real run: `make --script s.txt --plan` gives
`tts.kokoro` with this machine's data folder (Kokoro installed) and `tts.edge` with an empty `SCRIPTVOX_DATA`.
