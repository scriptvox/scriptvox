# Evals: scriptvox-arabic (run by hand: they cost model calls)

| Case | Pass when |
|---|---|
| en-to-ar | `lang` ar, an `ar-AE-*` voice, spoken words 43 to 53 with numbers counted as spoken; the agent dropped ideas instead of translating word for word; no Latin words except protected brand names |
| price | the prices stay exact (they are the point of the offer), in one digit style; the script is at most 22 spoken words with the prices counted as spoken; after the render the agent checked a frame with a price and saw it on the correct side |
