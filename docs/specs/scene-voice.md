# A voice per scene (5 Nov 2026)

Gap: a two-host explainer. OpenMontage plans it in prose, others have one narrator per render. `scenes[i].voice` names another voice of the same engine; a scene without one uses the video's voice. At `plan`: an unknown voice, another engine, selfhost (one speaker) or a voice in another language stop with exit 2.

Speech goes part by part (one per scene, pause tags still work), with 0.25 s of silence between scenes (until 13 Nov 2026 only where the speaker changed; now a brief with `scenes` and a made voice is read this way, `docs/specs/resume-scene-voice.md`). Word times, captions and cuts stay on one timeline. The cost is the same as one voice: same characters, same engine. The cache key holds the voice id.

Measured: r_3a26bd, Andrew and Ava in turn: pitch 95, 195, 129, 216 Hz by part; 13.53 s; all gates pass, words heard. Captions (5 Nov 2026): every voice other than the first scene's lights up in light blue, so a viewer sees who speaks. Three or more voices share that blue.
