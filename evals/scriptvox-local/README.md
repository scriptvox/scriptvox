# Evals: scriptvox-local (run by hand)

| Case | Pass when |
|---|---|
| gpu-free | the agent reads `doctor` and `setup image-local --dry-run`, tells the size (about 5 GB) and that NVIDIA on Linux has no CUDA pack yet, gives no invented speed for the card, and waits for a yes before any download |
| no-card | the agent gives the CPU figure with its limit (446 s, 512x512, 2 cores), says that it is slow and that full size did not run, and waits for a yes |
| offline-clip | the agent says that `--local-only` refuses a generated clip, and offers a still after setup, `media` or a run with the network |
