# Music library tracks

Traced on 17 Nov 2026 for MILESTONE-PARITY 12.5. ScriptVox ships no audio file: `registry/music.toml` holds the address and the sha256 of each track, and `make` downloads a track on first use.

How each track was traced:

1. The Wikimedia Commons file page carries the `{{cc0}}` licence tag and names its source: a Free Music Archive (FMA) track page.
2. That FMA track page states "CC0 1.0 Universal" and links `creativecommons.org/publicdomain/zero/1.0/`. This was checked for each of the 11 tracks.
3. The composers' own FMA artist pages say: Komiku, "Use it how you want"; Loyalty Freak Music, "Do whatever you want with this".
4. The file was downloaded, and its sha256 was computed and pinned. Its length was measured with ffprobe.

No statement about YouTube Content ID was found for either composer. CC0 gives no one a claim, but a third party can still register a CC0 song by mistake. If a claim comes, the source page in the credits is the proof.

| Name | Title | Composer | Mood | Length | File | Source | Licence | sha256 |
|---|---|---|---|---|---|---|---|---|
| `serenity` | Serenity Temple | Komiku | calm | 96 s | [Commons](https://commons.wikimedia.org/wiki/File:Komiku_-_34_-_Serenity_Temple.ogg) | [FMA](https://freemusicarchive.org/music/Komiku/Poupis_incredible_adventures_/Komiku_-_Poupis_incredible_adventures__-_34_Serenity_Temple) | CC0 1.0 | `1616dd9d8ac9…` |
| `quiet-saturday` | Quiet Saturday | Komiku | calm | 113 s | [Commons](https://commons.wikimedia.org/wiki/File:Komiku_-_57_-_Quiet_Saturday.ogg) | [FMA](https://freemusicarchive.org/music/Komiku/Poupis_incredible_adventures_/Komiku_-_Poupis_incredible_adventures__-_57_Quiet_Saturday) | CC0 1.0 | `9f8ffb49436f…` |
| `picnic` | Go to the Picnic | Loyalty Freak Music | happy | 187 s | [Commons](https://commons.wikimedia.org/wiki/File:Loyalty_Freak_Music_-_01_-_Go_to_the_Picnic.ogg) | [FMA](https://freemusicarchive.org/music/Loyalty_Freak_Music/POSITIVE_ATTITUDE_/Loyalty_Freak_Music_-_POSITIVE_ATTITUDE__-_01_Go_to_the_Picnic) | CC0 1.0 | `6cdccc32fb47…` |
| `sunflower-field` | Champ de tournesol | Komiku | happy | 118 s | [Commons](https://commons.wikimedia.org/wiki/File:Komiku_-_03_-_Champ_de_tournesol.ogg) | [FMA](https://freemusicarchive.org/music/Komiku/Its_time_for_adventure_/Komiku_-_Its_time_for_adventure_-_03_Champ_de_tournesol) | CC0 1.0 | `c1d514f13d63…` |
| `chill-out` | Chill Out Theme | Komiku | chill | 188 s | [Commons](https://commons.wikimedia.org/wiki/File:Komiku_-_02_-_Chill_Out_Theme.ogg) | [FMA](https://freemusicarchive.org/music/Komiku/Its_time_for_adventure__vol_2/Komiku_-_Its_time_for_adventure_vol_2_-_02_Chill_Out_Theme) | CC0 1.0 | `75cf291f5f2d…` |
| `take-a-break` | Take A Break | Loyalty Freak Music | chill | 126 s | [Commons](https://commons.wikimedia.org/wiki/File:Loyalty_Freak_Music_-_06_-_Take_A_Break.ogg) | [FMA](https://freemusicarchive.org/music/Loyalty_Freak_Music/ROLLER_DISCO_DANCE_DANCE/Loyalty_Freak_Music_-_ROLLER_DISCO_DANCE_DANCE_-_06_Take_A_Break) | CC0 1.0 | `d667016640ad…` |
| `beat-explosion` | High Technologic Beat Explosion | Loyalty Freak Music | energetic | 221 s | [Commons](https://commons.wikimedia.org/wiki/File:Loyalty_Freak_Music_-_02_-_High_Technologic_Beat_Explosion.ogg) | [FMA](https://freemusicarchive.org/music/Loyalty_Freak_Music/ROBOT_DANCE_/Loyalty_Freak_Music_-_ROBOT_DANCE__-_02_High_Technologic_Beat_Explosion) | CC0 1.0 | `63080c2e569a…` |
| `cant-stop-my-feet` | Cant Stop My Feet | Loyalty Freak Music | energetic | 186 s | [Commons](https://commons.wikimedia.org/wiki/File:Loyalty_Freak_Music_-_04_-_Cant_Stop_My_Feet_.ogg) | [FMA](https://freemusicarchive.org/music/Loyalty_Freak_Music/ROLLER_DISCO_DANCE_DANCE/Loyalty_Freak_Music_-_ROLLER_DISCO_DANCE_DANCE_-_04_Cant_Stop_My_Feet_) | CC0 1.0 | `8484df80ac0d…` |
| `tension` | Tension | Komiku | tense | 201 s | [Commons](https://commons.wikimedia.org/wiki/File:Komiku_-_12_-_Tension.ogg) | [FMA](https://freemusicarchive.org/music/Komiku/Its_time_for_adventure__vol_2/Komiku_-_Its_time_for_adventure_vol_2_-_12_Tension) | CC0 1.0 | `bdd3a9d7fc13…` |
| `sad-piano` | Sad walk with sad piano | Komiku | sad | 191 s | [Commons](https://commons.wikimedia.org/wiki/File:Komiku_-_32_-_Sad_walk_with_sad_piano.ogg) | [FMA](https://freemusicarchive.org/music/Komiku/A_Monplaisir_Best-Of__Lets_hear_that_crap/Komiku_-_A_Monplaisir_Best-of__Lets_here_that_crap_-_32_Sad_walk_with_sad_piano) | CC0 1.0 | `406180aa0399…` |
| `something-to-save` | Something to save | Komiku | epic | 211 s | [Commons](https://commons.wikimedia.org/wiki/File:Komiku_-_10_-_Something_to_save.ogg) | [FMA](https://freemusicarchive.org/music/Komiku/Its_time_for_adventure__vol_2/Komiku_-_Its_time_for_adventure_vol_2_-_10_Something_to_save) | CC0 1.0 | `ca0701814659…` |
