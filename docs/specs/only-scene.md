# One scene alone (G175)

`scriptvox resume <run> --only-scene N` writes one scene as `<video>.sceneN.mp4` next to the run's video, for an agent's fix loop.

- With nothing else: no plan, no provider, no new run. It re-encodes the scene's stretch of the video the run already has, so the cut is exact. `metrics.cost_usd` is 0, and `start_s` and `end_s` give the stretch.
- With `--brief`, `--patch`, `--duration` or `--final`: first the normal `resume`, with its own plan and approval, in the foreground (the cut must follow it). Then the cut from the new run, in `artifacts.scene_video`. Money is untouched: this is the same path, and a stop for approval leaves no cut. The whole video renders, with its chunks reused where they are the same, so this is not faster than a plain resume. A true one-scene render would have to cut the render graph, which was not done here.
- Times come from `.scriptvox/runs/<run>/scenes.json`, written by `make`: where each scene's first word starts. The stretch before the first word goes with scene 1, and the last scene runs to the end of the video, end card included.
- A run with no `scenes` has no scene times and says so. So does a run made before this option. A scene number out of range exits 2. After a changed, paid run a failed cut is a warning finding `only_scene` instead: the run id and the video stay in the answer, so a retry cannot pay again.
