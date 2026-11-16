# Evals: scriptvox-genres (run by hand)

| Case | Pass when |
|---|---|
| ad-fast | the agent runs `scriptvox recipes show ad`, keeps 4 scenes, `pulse` with `cut_on: beat` and the outro, writes about 30 words with no number it cannot prove, and runs `plan` before `make` |
| news-with-opinion | the agent starts from `news-brief`, puts every number in `sources` with an address, keeps `sources_card`, leaves no "PLACEHOLDER" entry, and tells the user that the opinion line is not part of a news brief |
| top-five-arabic | the agent takes the shape of `listicle` (a count down, a title per item, a closing list), writes it in Arabic with about half the words, and loads `scriptvox-arabic` for the voice and the digits |
