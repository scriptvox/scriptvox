# research: facts with their sources, before the script (13 Nov 2026)

Plan items P2.2 R1 to R5, R7, R9 and R10 of `docs/plan/MILESTONE-PARITY.md`. The text rows that it uses are in `docs/specs/write.md`. The search terms, the check that a page is about the topic, and the match of two values are in `docs/specs/research-search.md`.

Why: a video that states a number must be able to show where the number comes from. An agent can collect facts by itself. This command does it for a user who has no agent, and it checks in code what a model only promises.

This is an opt-in part. `make`, `plan` and every other command are as before: none of them calls a text model. Only `research` and `write` do, and only after the user added a text row.

```
scriptvox research "<topic>" [--lang en|ar] [--depth quick|normal|deep] [--out research.json]
                             [--url <address>]... [--text-row <name>] [--pages-only]
```

## 1. Flow

1. Check the flags. `--out` must not exist.
2. Without `--pages-only`: find the text row. No row: stop with exit 10 before any request (section 6).
3. A paid text row: stop for approval, or for the spend cap (`docs/specs/write.md`, section 3).
4. Search terms: one small call to the text row gives queries and subject names; with `--pages-only`, or when the call fails, the code makes them from the topic (`research-search.md`, section 1).
5. Search: each search row is asked with these terms (section 2).
6. Read: each result page and each `--url` page is read (section 3). A result page that is not about the topic is skipped before any call (`research-search.md`, section 2).
7. Extract: the text row gets the text of each page and gives facts. The code checks each fact against the page text (section 4).
8. The two-source rule gives each fact a status (section 5).
9. Write `research.json` (section 7) and return the envelope.

With `--pages-only`, steps 2, 3, 7 and 8 do not run, and step 4 is done by the code: the file holds the pages and no facts. This needs no text row. It shows what the search and the reader give.

`--plan` prints the rows, the limits and the cost, and sends nothing. `--local-only` stops the command: it needs the network.

| Depth | Results per search row | Pages read at most | Parts of one page sent to the model |
|---|---|---|---|
| `quick` | 2 | 3 | 1 |
| `normal` (default) | 4 | 6 | 2 |
| `deep` | 8 | 12 | 3 |

The text row gets at most one call for the search terms and one for each part of each page: 4 calls at `quick`, 13 at `normal`, 37 at `deep`. One part is at most 6,000 characters. The pages of `--url` (at most 10) are read first and count in the page limit.

## 2. Search rows

Search rows are data in `registry/search.toml`, as the provider rows are. The user's own rows are in `search.toml` in the config folder and come after the built-in rows. `SCRIPTVOX_SEARCH_ROWS` names another file in place of the built-in one (tests, mirrors).

| Field | Meaning |
|---|---|
| `name` | The row. |
| `key` | Empty: no key. Else a key name that the user declared (`docs/specs/key-names.md`). A built-in key name is refused, so a key of another service is never sent to a search host. |
| `auth_header` | The header that carries the key. |
| `search_url` | The request. `{query}`, `{lang}` and `{limit}` are filled in. |
| `near_url` | Optional: a second form of the request, for a name. It gives the page whose title is the query, or nothing, in the answer form of `search_url`. Only `wikipedia` has it. |
| `results` | Path to the list of results in the answer (the path form of `http_rows::get`). |
| `title` | Path, in one result, to its title. |
| `page_url` | The address of the page. `{lang}` and each `{field}` of the result are filled in. A field can be a path (`data[*].title`: the first match). |
| `text` | In place of a page read: the text of the result itself, from its fields. Markup in it (`<jats:p>`, `<h4>`, entities) is taken out. A result with no such text is skipped; its page is not read. |
| `publisher` | The name that the facts show. |
| `family` | Rows of one owner have one family. The two-source rule counts a family as one publisher. |
| `hosts` | The hosts that a page address of this row can have. |
| `langs` | The languages that the row has text in. The row is not asked for another language, and `skipped` says so. None: every language. |
| `good_for` | The kind of fact that the row is good for. `research --pages-only --plan` shows it. |
| `terms`, `terms_read` | What the publisher says about automated access, and the day it was read. Data for a person who checks a row. |

Built-in rows, all free and with no key, in the order of the file:

| Row | What it gives | Good for | Publisher, family | Languages |
|---|---|---|---|---|
| `wikipedia` | The search of the Wikipedia of the language, then the article page | General facts on any topic | Wikipedia, `wikimedia` | all |
| `nasa` | The NASA Image and Video Library: the caption of a result as text, no page read | Space, Earth and air science: sizes, distances, dates of missions | NASA, `nasa` | en |
| `europepmc` | Europe PMC: the abstract of a paper as text, no page read | Health, medicine and biology: measured values and study results | Europe PMC, `papers` | en |
| `doaj` | DOAJ: the abstract of an open-access article as text, no page read | Research results in any field | DOAJ, `papers` | en |
| `wikidata` | The entity search of Wikidata: label and description as text, no page read | What a thing is, in one line | Wikidata, `wikimedia` | all |

So the built-in rows are three publisher families, and a value that two of them give is `confirmed` with no `--url`. `europepmc` and `doaj` are one family: the two can give the same paper, and one paper must not confirm itself. This helps a science topic. For history, people, places or sport the three new rows find little: `--url` is still the sure way to a second publisher.

The page address of a result of the three rows is in the fact and goes to `sources`, but the tool does not read it. Asked one time each on 13 Nov 2026 with a plain client: NASA 200 (the page shows its text only with JavaScript), Europe PMC 403, DOAJ 403. A person's browser opens them. `export --check-links` names the two that answer 403.

How the three rows were chosen (13 Nov 2026). Each candidate got a real request with no key, and its public terms were read. The terms note and the date are in the row. In that test one request went to each row. A run now sends at most 7 requests to a row, one at a time with a pause of 0.4 s, with the `User-Agent` of the tool and nothing about the user (`research-search.md`, section 1.3).

| Candidate | Result |
|---|---|
| NASA Image and Video Library | In. No key and no rate limit is stated for `images-api.nasa.gov`. NASA asks to be named as the source. The page `api.nasa.gov` could not be read. |
| Europe PMC | In. No key and no rate limit is stated; EMBL-EBI blocks a use that hinders the service and expects attribution. The page `europepmc.org/RestfulWebService` refused a plain client; `dev.europepmc.org` and the EMBL-EBI terms were read. |
| DOAJ | In. No key for a search, at most two requests a second, metadata under CC0. |
| Crossref | Out. An abstract comes only with a filter and with markup, at one request a second; it is the same kind of text as the two rows above. Its terms allow the use. |
| Internet Archive | Out. Its terms of use page could not be read (it needs JavaScript), and the results were noisy. It was the only candidate with Arabic results. |
| Open Library | Out. No text of a fact in the answer, and its pages ask "are you a human". |
| GOV.UK search | Out. The API is "unsupported and may change", and it is for one country. |
| Library of Congress | Out. The JSON search answered 403 with a challenge page. |
| OpenAlex | Out. The abstract is a word index and not text, and the key-less budget is small. |
| arXiv | Out. The answer is Atom XML, and a row reads JSON. |

The page slots go to one result of each row in turn. So with five rows, `quick` (3 pages) reads one page of `wikipedia`, `nasa` and `europepmc`, and `normal` (6) reads one of each row and a second of `wikipedia`. For more Wikipedia pages use `deep`, or your own row file.

A page that is not about the topic gives its slot to the next result of the rows.

A row that fails (no answer, bad JSON) is named in `skipped` and the others go on. A row that fails after some requests keeps the results of those requests. All rows fail and no `--url`: exit 5.

## 3. The page reader

- One `GET` for each page. Time limit 20 s. At most 2 MB is read. Only `text/html` and `text/plain`; an answer with no content type is read as HTML.
- The address must be `https`. `http_rows::local_host` must say no: the same guard as `fetch` against a private, loopback or link-local address and against a name that resolves to one. A redirect is followed by hand, at most 5, and each new address gets the same two checks.
- The `User-Agent` is `scriptvox/<version>` with the address of the project. No cookie, no referrer, nothing about the user or the machine.
- A page that is refused or fails is in `skipped` with its cause. The address of a refused private page is not requested.
- A page that does not connect, does not answer in time or stops before its end gets one more try after 1 s. The cause then says so: "no connection, two times: the site refused it or closed it before an answer", "no answer in 20 s, two times". Other causes are not tried again: "the host name was not found: check the address", "the secure connection failed (certificate or TLS)", `HTTP <code>`.
- Main text only: the first `<main>` element, else the first `<article>`, else `<body>`. Scripts, styles, navigation, headers, footers, side parts, forms, tables, figures, reference marks and edit links are left out. A line under 40 characters is left out (menus, headings). A raised number is kept with `^` before it, so "10^9" does not become "109".
- Title: the search result's title, else `<title>`. Publisher: the row's `publisher`, else `og:site_name`, else the host. Page date: `article:modified_time`, `article:published_time` or `date` of the page, else the `Last-Modified` header, else null. The header can be the day of the request (Wikipedia, 13 Nov 2026), and `limitations` says so. The page address keeps its query and loses its fragment.
- A page with less than 200 characters of main text is skipped.

Test variable: `SCRIPTVOX_RESEARCH_BASE` is a list of `http://127.0.0.1:<port>` bases, separated by commas. Only an address that starts with one of them passes the guard as a test server. Every other private address is refused also in a test.

## 4. Fact extraction and the sentence check

The text row gets one part of one page and must answer with JSON: a list of facts, each with `text` (the fact in one sentence), `sentence` (copied from the page, word for word), `value` (the number or date of the fact, if it has one) and `subject` (a short name of what the value measures). The page text is data: the prompt says that instructions inside it must be ignored. The names of the subjects found so far go to the next request, so that the same quantity gets the same name on a later page.

The code does not trust the answer. A fact is kept only when all of these are true:

| Check, in code | If not |
|---|---|
| `sentence` is found in the page text, and what is found is 20 to 600 characters. Both sides are folded for the match only (see below) | dropped, counted as `no_sentence` |
| Each number of `text` is also a number of `sentence` | dropped, counted as `number_not_in_sentence` |
| `text` is 1 to 300 characters | dropped, counted as `bad_form` |
| `value`, if given, is a number or a date that is in `sentence` | the value is removed; if `text` has exactly one number, that number is the value |

At most 8 facts for one part. A second fact with the same text from the same page is dropped. The same text from a second page stays: that is how two publishers confirm.

The fold (`claims::fold`). A model does not copy a sentence byte for byte: it changes a digit form, drops a mark or a tatweel. So the match is made on folded text: each run of white space is one space; compatibility forms are made plain (NFKD: a presentation form of an Arabic letter, a full-width digit); combining marks are removed (the harakat, the hamza on an alef, a Latin accent); the tatweel, zero-width and direction marks are removed; Arabic-Indic and Persian digits and `٪` become ASCII; the Persian yeh and kaf and the alef maksura become the Arabic letters; curved quote marks and long dashes become plain ones. Letter case is not folded. The fold is for the match only: `sentence` in the file is the text of the page as the page has it, cut out at the place of the match, not the model's copy.

A quotation. The model can give `quote` with `text` and `speaker`. The code keeps it only when the sentence has these words between quotation marks, and has the name of the speaker outside the marks (10 to 160 characters, a name of 2 to 40). Both values in the file are cut from the sentence. `write` makes a quote card only from such a fact.

A value in `research.json` has `kind` (`number` or `date`), `number` and `unit` or `date`, and `raw` (the text as found). Numbers: `1,234.5`, `42%` and `42 percent`, `3 million` (thousand, million, billion, trillion), Arabic-Indic and Persian digits, `٤٢٪`, `42 في المئة`, `3 ملايين` (ألف, آلاف, مليون, ملايين, مليار, مليارات, تريليون), and a number joined to an Arabic word (`و3`, `الـ20`). Dates: a year (1000 to 2100), `12 March 2020`, `March 12, 2020`, `2020-03-12`, and `12 مارس 2020` with an Arabic month name of one word (both sets: يناير, and شباط).

## 5. The two-source rule (R5)

Only for a fact with a value. The rules of "the same value" and "one subject" are in `research-search.md`, section 5.

| Two facts of two publishers | Status |
|---|---|
| The same value, and one subject (the same key terms, or the shorter name inside the longer one) | `confirmed`, with `confirmed_by` (the ids of the other facts) |
| Exactly the same key terms of the subject, the same unit, and values that are not the same | `conflict`, with `conflicts_with`. Both facts stay in the file, each with its own value. |
| Else | `single` |

A fact with no value is always `single`: the code cannot compare two sentences.

A publisher is the row's `family`, else the host of the page without `www.`. So Wikipedia and Wikidata are one publisher, and so are two languages of Wikipedia. A `--url` page on a host of a row has the family of that row: a `--url` on Wikipedia does not confirm the `wikipedia` row. A second publisher comes from another built-in row, from `--url`, from a search row of the user, or from the agent.

The subject names are compared by their key terms in folded form, so a name with and without a hamza, with and without the article, or with its words in another order is one subject. The unit is compared by a table of unit names: `450 nm` and `450 نانومتر` are one value. Two numbers agree when one is the other one rounded and they are at most 0.5 percent apart.

## 6. Errors

All use an existing exit code.

| Cause | Exit | Code | Fix |
|---|---|---|---|
| No text row (and no `--pages-only`) | 10 | `SVX_NEEDS_INPUT` | `scriptvox connect local --url http://127.0.0.1:11434 --text --model <id>` (a local server), or the address of a service and `--key-name`. Or `--pages-only`. |
| `--text-row` names no row | 2 | `SVX_USAGE` | The names of the rows. |
| The row's key is not set | 10 | `SVX_NEEDS_INPUT` | `scriptvox keys set <name>` in the user's terminal. |
| A paid row, not approved | 4 | `SVX_APPROVAL_NEEDED` | Ask the person, then `--approve <plan_id>`. |
| A paid row, cost above `--max-spend` | 4 | `SVX_SPEND_CAP` | Raise the cap after the person says yes. |
| Empty topic, topic above 200 characters, bad `--lang`, `--out` exists, more than 10 `--url` | 2 | `SVX_USAGE` | Names the flag. |
| `--url` is not `https`, or is a private address | 2 | `SVX_RESEARCH_PRIVATE` | Give a public https address. |
| `--local-only` | 2 | `SVX_USAGE` | Run without `--local-only`. Nothing was sent. |
| No search row answered and no page was read | 5 | `SVX_PROVIDER_RETRYABLE` | Run the command again, or give pages with `--url`. |
| The search answered and no page could be read | 1 | `SVX_RESEARCH_NO_PAGES` | Use other words for the topic, or give pages with `--url`. `skipped` names each cause. |
| Pages were read and none is about the topic | 1 | `SVX_RESEARCH_NO_PAGES` | The message names the pages. Name a page with `--url`, or give the topic as a name and not as a question. No page went to the text row. |
| A text row on a network service with no price | 4 | `SVX_APPROVAL_NEEDED` | The cost is not known: ask the person, then `--approve <plan_id>`. |
| The text row does not answer, or answers with an HTTP error | 5 | `SVX_PROVIDER_RETRYABLE` | Start the server or check the row; run again. |
| Pages were read and gave no fact | 1 | `SVX_RESEARCH_NO_FACTS` | The message names the pages. When they are not about the topic: name a page with `--url`, or give the topic as a name and not as a question. When they are: `--depth deep` or a larger model. The file is written with the pages and the counts. |

An answer of the model that is not JSON costs only that part of that page: it is counted in `dropped.bad_answer`.

## 7. `research.json`

```json
{
  "schema": "scriptvox.research/v1",
  "topic": "why the sky is blue", "lang": "en", "depth": "normal",
  "read_at": "2026-11-13",
  "text_row": {"name": "local", "model": "..."},
  "search": {"by": "text row", "queries": ["sky colour scattering", "sky blue"], "names": ["Rayleigh scattering"],
             "key_terms": ["sky", "blue"]},
  "pages": [
    {"id": "p1", "url": "https://en.wikipedia.org/wiki/Rayleigh_scattering", "title": "Rayleigh scattering",
     "publisher": "Wikipedia", "family": "wikimedia", "page_date": "2026-10-30", "read_at": "2026-11-13",
     "via": "wikipedia", "chars": 21840}
  ],
  "facts": [
    {"id": "f1", "text": "Blue light is scattered more than red light.",
     "value": null, "subject": "scattering of blue light",
     "url": "https://en.wikipedia.org/wiki/Rayleigh_scattering", "page_title": "Rayleigh scattering",
     "publisher": "Wikipedia", "page_date": "2026-10-30", "read_at": "2026-11-13",
     "sentence": "...the exact sentence of the page...",
     "status": "single"}
  ],
  "dropped": {"no_sentence": 2, "number_not_in_sentence": 0, "bad_form": 0, "bad_answer": 0},
  "skipped": [{"what": "wikidata", "why": "no result"}],
  "limitations": ["..."]
}
```

The envelope gives `artifacts.research`, and in `metrics`: `search_results`, `pages_read`, `pages_skipped`, `page_chars`, `facts`, `confirmed`, `single`, `conflict`, `dropped`, `calls`, `cost_usd`. It holds no page text.

An agent with its own web search can write this file by itself: `write --from` reads it the same way (it needs `schema`, `topic`, and for each fact `id`, `text`, and `url` or `page_title`), and gate `claims` checks the brief that comes out.

## 8. Gate `claims` (R7), in `plan` and `check`

No model. Plain text and number matching.

- The gate runs only when the brief has `sources`. A brief with no `sources` gets no finding, as before.
- The text that is checked: the script (all scenes or lines), the hook and the text of the cards.
- A claim is a number written in digits (with the forms of section 4), or a month name next to a number.
- A claim matches when the same number, or the same month, is in the `text` of one entry of `sources`, in any digit form (`٤٥٠` in the script matches `450` in a source). A number in the name of a source also counts (`publisher`, else `title` when there is no `publisher`), so the name "France 24" on a Sources card is not a claim with no source. This is new for a brief whose source has a `title` with a number and no `publisher`.
- New on 13 Nov 2026: a number joined to an Arabic word (`و3`, `الـ20`) is a claim. Before, it was not read. An Arabic brief with `sources` can get a warning that it did not get before.
- All match: `pass`. One or more do not: `warn`, and the finding names them (at most 12).
- `--strict` on `plan`, `make` and `check` makes the warning a failure: exit 3, and `make` starts no work.
- `check <run>` reads the brief of the run. `check <file>` has no brief and gives no finding.
- `wait <run>` shows the same finding for a run that has ended (13 Nov 2026: it was missing, since the result file of a run is written before `make` adds the finding). Never a failure there: `--strict` stops a run before it starts. Test `wait_shows_the_claims_finding_of_the_run`.

What the gate cannot see is in `limitations`: names and facts with no number are not checked; a number written as a word ("three") is not seen, so write a count that is not a fact as a word; a match means that the number is in a source text, not that the two sentences say the same thing; an Arabic month name of two words (كانون الثاني) is not seen.

## 9. Sources in the result (R8) and the link check (R9)

R8 exists since `docs/specs/brief-sources.md`: `export` writes the sources to `metadata.json` and `credits.txt`, and the recipe in the MP4 holds them. The second part of R8, a card "Sources" on the screen, is the brief field `sources_card` (`docs/specs/sources-card.md`).

R9: `export <run> --check-links` asks each source address one time (`HEAD`, then a small `GET` if the server refuses `HEAD`; the guard and the limits of section 3). The result is the finding `links`: `pass`, or `warn` with each dead address and its cause, and the list `links` in the envelope. An address that is not `https`, or is private, is not asked and is named. It is behind a flag because it needs the network; with `--local-only` nothing is asked and the finding says "not checked". A dead link does not fail the export.

## 10. For an agent (R10)

`research` and `write` are MCP tools. A text row is added with `connect --text`; the value of a key goes in only through `scriptvox keys set`, which the person runs. The skill says: research first, then write, then `plan` and read the `claims` finding.

## 11. Not done

- `providers --json` lists the text rows and the faults of a bad `[[text]]` block in `text_rows`. The search rows are not listed by a command yet.
- Search rows with a key: the row format carries a key, and no such row is built in.
- Arabic has one publisher family in the built-in rows (Wikipedia and Wikidata in Arabic). The three other rows are English only. No key-less Arabic source with readable terms was found on 13 Nov 2026, so an Arabic fact is `confirmed` only with `--url` or a row of the user.
- The three new rows are for science. A topic of another kind gets little from them, and they take page slots from Wikipedia at `quick` and `normal`.
- No fact was seen to become `confirmed` with a real model: two real runs on 13 Nov gave none. After them the match was changed (`research-search.md`, section 5); the new match ran against the fake server only.
- The fold does not fold letter case, and does not join a letter with a combining mark into one letter before the match (it removes the mark on both sides, so "é" and "e" are one letter for the match).
- No JavaScript: a page that builds its text in the browser gives little or no text.
- The main-text rule is simple. It is not a full reader mode, and tables are left out.
- Only UTF-8 pages are read correctly.
- PDF files are not read.
- The two-source rule depends on the `subject` name that the model gives. The code compares the key terms of two names, so another word order or another small word is one subject. Two names with other words for one quantity stay `single`. The values are compared in code.
- A fact with no number has no second check: its sentence is in the page, and the `text` is the model's wording of it.
- Names in the script are not checked by `claims`.
- No source line under each scene. The "Sources" card is one card on the last scene (`sources-card.md`).
- The cost of a paid row is counted from the `usage` field of the answer; with no such field, from the character count.

## 12. Arabic

`research "<topic in Arabic>" --lang ar` asks the Arabic editions: `{lang}` in the address of the `wikipedia` row and in the language of the `wikidata` row. A row with `langs` that has no `ar` is not asked. The file has `lang: ar`, the model is told to write the facts in Arabic, and `write --from` takes the language of the file, so the brief has `lang: ar`.

The sentence match, the number check of a fact and gate `claims` read Arabic text as section 4 says: one number in any of the three digit forms, with or without a tatweel before it, and the harakat do not break a match.

Real run, 13 Nov 2026, no model (`--pages-only`), topic "لماذا السماء زرقاء": 4 results, 4 pages of the Arabic Wikipedia read (9,660 to 124,958 characters of main text), 3 rows not asked (English only), Wikidata gave no result. A run with a real model on the same evening showed that the pages were not about the topic; `research-search.md` has what was changed and the runs after it.
