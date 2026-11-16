# research and write: search terms, the topic check, numbers and cards (13 Nov 2026)

Five faults from the second run with a real text model (`docs/daily/2026-11-13.md`). This file says what changed. `research.md` and `write.md` hold the rules of the two commands.

| Fault in the real run | Now |
|---|---|
| The whole question went to the full-text search. In Arabic it found a folk character and a song | Section 1: search terms from the text row, else from the topic in code; a title form of the search |
| The two wrong pages went to the paid model | Section 2: a page is checked for the topic in code before a call |
| The error said only "no fact passed the checks" | Section 3: the error names the pages and the two ways out |
| `write` used no number and gave only a title card | Section 4: a number and a card with a number are a must, with one more question |
| No fact was `confirmed` in two runs | Section 5: values and subjects are compared by rule, not as text |

## 1. Search terms

### 1.1 With no model (the code)

Used with `--pages-only`, and when the call of 1.2 fails.

- **Key words.** The topic without question words and stop words. The two lists are closed and short: English ("why", "how", "what", "is", "the", "of") and Arabic ("لماذا", "كيف", "ما", "هل", "في", "من"). The full lists are `STOP_WORDS` in `research.rs`. A word that says no ("not", "لا", "لم") is not a stop word: it changes what a subject name measures, and a name with such a word never agrees with a name without it (section 5).
- **Queries**, for the full-text form of a row (`search_url`): the topic as it is, then the key words. A topic with no stop word is a name ("Rayleigh scattering"): one query.
- **Names**, for the title form of a row (`near_url`): the topic when it is a name. Else each key word of the question, at most 3: a question names its subject with one of its words ("sky", "vaccines", "السماء").
- **Order of the pages of a row:** the results of the question first, in their order. This is what a run gave before, so no topic gets worse. Then the first result of each other request in turn (the key words, then each key word as a title), then the second results. A page of these later requests is read when the page limit has room, and when the check of section 2 takes out a page of the question: a wrong page from the question has few of the key words, so the check finds it. The code cannot know which word of a question is its subject ("sky") and which is not ("blue"), so no title page comes first. For a topic that is a name, the title page comes first.

A row can have a title form. It is the new row field `near_url`: the same answer form as `search_url`, and it gives the page whose title is the query, or nothing. The `wikipedia` row has it: `list=search` with `srwhat=nearmatch` (public API help, module `query+search`). A redirect counts, so "Earthquakes" gives the article "Earthquake". The other built-in rows have no title form and get only the queries.

### 1.2 With the text row

One small call before the search (at most 300 tokens out). The model gives:

- `queries`: at most 3 search queries of 1 to 5 words;
- `subjects`: at most 3 exact names of the things that explain the topic, as an encyclopedia names its article ("Rayleigh scattering" for "why is the sky blue").

Both are in the language of `--lang`. The code keeps strings of 1 to 80 characters, each one time, and adds its own key-word query. The question itself is not sent. The subjects go to the title form, the queries to the full-text form. Results are merged by page address. Order of the pages of a row: the title match of a subject first, then by the number of requests that found the page, then by its best place.

When the call fails, or its answer has no terms, the terms of 1.1 are used. `skipped` then has an entry `search terms` with the cause, and the run goes on. The call is counted in `metrics.calls` in both cases.

`research.json` has the new field `search`: `by` (`text row` or `code`), `queries`, `names`, `key_terms`.

### 1.3 Requests, and what a search host can answer

Before, one row got one request in a run. Now a row gets at most 7: the names (when it has a title form), then the queries. On a public host there is a pause of 0.4 s between two requests to one row. When a request fails (HTTP 429 is "too many requests"), the row is not asked again in this run, the results that it gave are kept, and `skipped` names the fault.

A request that gets no answer in 20 s is tried one time over IPv4, as a request that does not connect (seen on 13 Nov 2026: the IPv6 route took requests and gave nothing back).

### 1.4 The key-less test (13 Nov 2026)

5 English and 5 Arabic topics, each as a question. No key and no model. First the modes of the Wikipedia search were tried one at a time, 3 results each, with a script outside the tool:

| Mode | Result |
|---|---|
| The question as it is, full text | English: right page in the first 3 for 4 of 5 topics. Arabic: 2 of 5; "لماذا السماء زرقاء" gave "جحا" and "ابن القمر (أغنية)" |
| Key words, full text | English: worse for "sky blue" (the colour, a record, a film studio). Arabic: better for 2 topics ("الإمبراطورية الرومانية" first), and "السماء زرقاء" gave three films |
| `srwhat=nearmatch`, the question | 1 hit of 10 (a redirect with the question as its title) |
| `srwhat=nearmatch`, the key words together | 2 hits of 10, one right ("Roman Empire fall") and one wrong ("Sky blue") |
| `srwhat=nearmatch`, each key word | The article of the subject for 8 of 10 topics ("Sky", "Vaccines", "Earthquakes", "Everest", "السماء", "اللقاحات", "الزلازل", "إيفرست"); not for the Roman Empire in both languages, where the subject has two words ("Roman", "Empire"). Also the articles of the other words ("Blue", "Work", "Tall", "Fall") |
| `srwhat=title` | Not offered by the search engine of Wikipedia (an error answer) |
| `srqiprofile=popular_inclinks_pv` | Famous pages that are not about the topic. Not used |

So no one mode is enough, and the code uses three together: the question as a query, the key words as a query, and each key word as a title. A first build wanted the other key words in a page that was found by one word as its title. In a real run it threw away the right page "Vaccines" (its start has no "work"), so that rule was taken out: a right page must not be lost, and a wrong one costs one call.

Wikipedia answered HTTP 429 after about 12 requests in 8 seconds from a plain client. This is why 1.3 has a limit and a pause.

Then the tool itself, `research "<topic>" --lang <l> --pages-only` (depth `normal`, 6 pages at most), build 9859c3e against this branch. The table is in section 7.

## 2. The topic check

Before the text of a page goes to the model, the code looks for the key terms of the topic in the title and in the first 1,500 characters of the page. No model.

- **Key terms:** the content words of the topic, folded as `claims::fold` does (harakat, tatweel, letter forms, digits) and in lower case. An Arabic word loses its article, also with a joined و, ب, ك, ف or ل. An English word loses its plural "s".
- **A term is in the page** when: an Arabic term is a word of the page or starts one, after the word lost its article, or a word of the page has the same consonants (so "زرقاء" finds "الأزرق"). It is not looked for inside a word: "سماء" is in "أسماء" (names). Another term is a word of the page with or without its plural "s" ("mar" finds "Mars"), or starts a word ("vaccine" finds "vaccination"); a term under 4 letters is the whole word.
- **The rule is low:** a page passes with one third of the key terms, rounded up. So 1 term of 2 or 3 is enough, and 2 of 4.
- **The subject names of 1.2 count.** A page that holds all key terms of one subject name passes.
- **Always read, with no check:** a page that is the title match of a subject name of 1.2, and a page that the person names with `--url`. So "Rayleigh scattering" is read for "why is the sky blue", though its start holds neither "sky" nor "blue".

A page that does not pass is in `skipped` with the reason "not about the topic: 0 of 2 key terms of the topic are in its title and its first 1500 characters". It costs no call, and its slot goes to the next result. At most 3 times the page limit are read in one run. A second title of an article that is in the list already (a redirect) is skipped as "the same text as the page ...".

The check runs with `--pages-only` also, so it can be tried with no model.

What the check cannot do: it reads words, not meaning. A page about another thing with the same words passes ("Sky blue", the colour, for "why is the sky blue"; with no model also "Blue" and "Work", the pages of one word of the question). A right page whose start has none of the words is lost when the text row did not name it; the two ways out of section 3 are for this case.

## 3. Errors that say what to do

| Cause | Exit | Code | Message | Fix |
|---|---|---|---|---|
| Pages were read and none is about the topic | 1 | `SVX_RESEARCH_NO_PAGES` | "no page about the topic was found: 2 pages were read and none is about it ("جحا", "ابن القمر (أغنية)"). No call was made to the text row for them." | "Name the page: add --url <address of a page about the topic>. Or give the topic as a name and not as a question (for example "Rayleigh scattering", not "why is the sky blue")." |
| Pages were read and gave no fact | 1 | `SVX_RESEARCH_NO_FACTS` | "2 pages were read and gave no fact for the topic: "A", "B". 0 facts of the model were dropped by the checks, 0 answers were not JSON." | "If these pages are not about the topic: " and the two ways out. "If they are: try --depth deep or a larger model. The file holds the pages and the counts." |

At most 8 titles are named. In the first case no file is written, and `metrics.calls` and `metrics.cost_usd` say what the search-terms call cost. `make --topic` has no `--url`: its fix line gives the way through `research "<topic>" --url <address>`, `write --from` and `make --brief`, and then its own last sentence, as before.

## 4. `write`: a number and a card with a number

When the research file has a fact with a number or a date that is not in conflict, the script must have two things:

1. **A number said.** One scene or more says the value of such a fact and lists the fact. The check is the number match of gate `claims`: the value of the fact is found in the hook or the scripts in any digit form.
2. **A card with a number.** One scene or more has a `stat` or a `bars` card that the code could build.

The code asks only for what it can make. A fact counts for 1 when its value is in its `text`. A fact counts for 2 when the code can build a `stat` card from it. The code still builds each card from the fact, never from the model's text.

A `stat` card now takes a fact with a date also: a year is a value that the page shows. The fact list shows such a fact with the mark `[date 1871]`.

The prompt states the two things as a must. When an answer is valid and does not have them, the model is asked one more time. The message says exactly what is missing and names facts that can give it:

- "Your last answer had no script says a number or a date of a fact. Say the value of one of these facts in a script, in digits as the fact writes it, and list the fact in `facts` of that scene: f1 (450 nm), f6 (78%)."
- "Your last answer had no card with a number. Give one scene this card, with the id of one of these facts: {"kind":"stat","facts":["<id>"]}. The facts: f1 (450 nm). Your card was not made: scene 2, stat: the fact f5 has no number."

**The call bound does not change: at most 3.** The length and the numbers share the one more question: one message holds the word count and what is missing. Of two valid answers the one with fewer things missing is taken (the length is one thing), then the one nearer to the length.

After that the answer is taken, never in silence. The envelope has the finding `cards`:

- `warn`: "no card with a number, though the research file has such facts. The model was asked one more time and did not give it", with a fix (add the card by hand, or a larger model). The same finding names a script that says no number.
- `pass`: "a script says a number of a fact, and a card shows a number of a fact".

A script from `--topic` has no facts and no such rule.

## 5. `confirmed`: the same value of one subject

Before, two facts confirmed each other only when the model gave both the same subject name, letter for letter, and the same unit text. Now the code compares by rule. No model.

**The same value.**

- **Unit.** A table of unit names, English and Arabic (`UNITS` in `research.rs`): percent (`%`, `per cent`, `بالمئة`, `في المئة`, `بالمائة`), nm (`nanometre`, `nanometer`, `نانومتر`), µm, mm, cm, m (`metre`, `meter`, `متر`), km (`kilometre`, `كيلومتر`, `كم`), km/s, km/h, kg, °C, year (`years`, `سنة`, `سنوات`, `عام`), day, people (`نسمة`). A unit is compared as a whole: `km/s` is not `km`. A unit that is not in the table is compared as text. There is no conversion: 450 nm and 0.45 µm do not agree.
- **Number.** The same number. Or two numbers of which one is the other one rounded: they differ by at most half of the last shown digit of the less exact one, **and by at most 0.5 percent** of the larger one. So 78 and 78.08 percent agree, and 8,849 and 8,848.86 m. 299,792 and 300,000 km/s do not agree (the last shown digit of both is 1), and 8,848 and 8,849 m do not.
- **Date.** Exact, as before: a year alone agrees with a date in that year, and with no other year.

**One subject.** The key terms of the two subject names (section 2: no stop words, no article, no plural "s", any order):

- the same set of terms; or
- all terms of the shorter name are in the longer one, and the shorter name has two or more terms.

So "wavelength of blue light" and "blue light's wavelength" are one subject, and "share of nitrogen" and "share of nitrogen in dry air". One shared word is not enough ("share of nitrogen", "share of oxygen"), and all words but one are not enough ("wavelength of blue light", "wavelength of red light").

**Conflict** is stricter than before in nothing and looser in one thing: the two names must have exactly the same key terms (before: the same text), the same unit by the table, and values that are not the same. A longer name with another number is not a conflict: it can be another quantity. So two facts for a `bars` chart are never put in conflict.

A wrong `confirmed` is worse than a missed one. What still stays `single`: two names for one quantity with a word that the other does not have ("share of nitrogen in air", "nitrogen in the atmosphere"); a subject in English and the same in Arabic; a value in another unit.

## 6. Call and cost bounds

| Command | Calls at most, before | Now |
|---|---|---|
| `research --depth quick` | 3 | 4 |
| `research --depth normal`, and the research step of `make --topic` | 12 | 13 |
| `research --depth deep` | 36 | 37 |
| `write` | 3 | 3 |

The new call is the search-terms call. The plan counts it as a call of the largest size (9,400 characters in, 1,500 tokens out), so the cost bound is `calls * (9,400 / 2 * usd_per_1m_in + 1,500 * usd_per_1m_out) / 1,000,000`: for prices of 1 and 2 USD for a million tokens, 0.0308 USD at `quick`, 0.1001 at `normal`, 0.2849 at `deep`. The real call is small (about 150 tokens in, at most 300 out). The plan id does not change, because the inputs of the work are the same.

## 7. Real runs with no key (13 Nov 2026)

`research "<topic>" --lang <l> --pages-only`, depth `normal` (6 pages at most), no text row, no key. "After" is the last build of this branch, one run for each topic. "Before" is build 9859c3e (with the one-line IPv4 retry of 1.3, because the IPv6 route failed that evening). The table has the pages of the `wikipedia` row in the order read; the other rows are counted. In Arabic only `wikipedia` and `wikidata` are asked, and `wikidata` gave no result.

| Topic | Before: Wikipedia pages read | After: Wikipedia pages read | After: skipped as "not about the topic" |
|---|---|---|---|
| why is the sky blue | Diffuse sky radiation; Sky blue (and 4 of other rows) | the same two (and 4 of other rows) | none |
| how do vaccines work | Nasal vaccine; Anti-vaccine activism; Vaccine hesitancy (and 3) | Nasal vaccine; Anti-vaccine activism (and 4) | none |
| what causes earthquakes | Earthquake; List of earthquakes in New Zealand (and 4) | Earthquake; Namazu (and 4). The search engine gave another second result in this run | none |
| how tall is mount everest | Mount Everest in 2018; Mount Everest; Tallest mountain; 1996 Mount Everest disaster (and 2) | Mount Everest in 2018; Tallest mountain; Mount Everest; 1996 Mount Everest disaster (and 2) | 2 abstracts of `europepmc` |
| when did the roman empire fall | The History of the Decline and Fall of the Roman Empire; Fall of the Western Roman Empire (and 4) | the same two (and 4) | none |
| لماذا السماء زرقاء | جحا; ابن القمر (أغنية); ديابوليك لافرز; المسيحية في الولايات المتحدة | السماء الزرقاء (فلم); السماء; زرقاء; فافنر في السماء الزرقاء; سماء مونتانا الزرقاء; اليوم الدولي لنقاوة الهواء من أجل سماء زرقاء | all 4 pages of "before" |
| كيف تعمل اللقاحات | درو وايزمان; لقاح مستضد الورم; التأثير الحضاري للمسيحية; علم المناعة | درو وايزمان; لقاح مستضد الورم; علم المناعة; اللقاحات النباتية; اللقاحات; لقاح كوفيد-19 | التأثير الحضاري للمسيحية |
| ما سبب الزلازل | زلزال; مقاييس الزلازل; هندسة زلازل; التنبؤ بالزلازل | زلزال; هندسة زلازل; مقاييس الزلازل; التنبؤ بالزلازل; سبب | none ("الزلازل" skipped as the same text as "زلزال") |
| كم يبلغ ارتفاع جبل إيفرست | جبل إفرست; معسكرات جبل إيفرست; جبل; جبل أوليمبوس (المريخ) | جبل إفرست; معسكرات جبل إيفرست; جبل; جبل أوليمبوس (المريخ); إيفرست | none |
| متى سقطت الإمبراطورية الرومانية | إمبراطوريات الشرق الأوسط; قائمة أفلام عقد 1990 المبنية على أحداث حقيقية; معركة أوسترليتز; تاريخ سوريسن | إمبراطوريات الشرق الأوسط; معركة أوسترليتز; تاريخ سوريسن; الإمبراطورية الرومانية; الإمبراطورية; الرومانية | قائمة أفلام عقد 1990 المبنية على أحداث حقيقية |

What the table says, read from the titles only (no person and no model read the pages):

- **English: no change in the pages of the question.** The search engine of the English edition already ranks a question well. The new requests find the pages of the subject ("Sky", "Vaccines", "Everest"), and at depth `normal` they get no slot, because five rows share six pages.
- **Arabic, the fault of the real run:** the four pages of "لماذا السماء زرقاء" are all skipped by the check with 0 of 2 key terms, at no cost. The article "السماء" is read now. Four other pages pass that only share the words (three films and a day of the United Nations). The page on Rayleigh scattering is not found with no model: its title holds no word of the question.
- **Arabic, the other topics:** the article of the subject is new in two of four ("اللقاحات", "الإمبراطورية الرومانية"), and was there before in two ("زلزال", "جبل إفرست"). Two wrong pages are skipped. Pages of one word of the question come in also ("سبب", "جبل", "الإمبراطورية", "الرومانية"): the check cannot stop them.
- **A first build of the check** read a term inside a word. The page "جحا" passed, because "سماء" (sky) is inside "أسماء" (names). Now a term is matched as a word.

The second way out, a topic as a name (two more runs): `"Rayleigh scattering"` read "Rayleigh scattering" and "Mie scattering" (and 4 of other rows), nothing skipped. `"تبعثر رايلي" --lang ar` read "تبعثر رايلي", "ظاهرة تندل", "تبعثر" and "لون العين".

Counts of the 12 runs: 5 to 38 search results, 5 or 6 pages read, 16,257 to 350,243 characters of main text. One run asks the `wikipedia` row 2 to 5 times.

## 8. Tests

No network; fake servers on loopback (`crates/scriptvox/tests/research.rs`) and unit tests (`research.rs`, `write.rs`).

| Point | Test |
|---|---|
| 1.1, 1.2, 1.3, 2 | `search_terms_come_from_the_text_row_and_a_page_is_checked_for_the_topic` (the requests that are sent, a page of two requests before a page of one, a row that answers HTTP 429 and keeps its results, the page that is skipped and costs no call, the fall back, the call bound of the plan); the cost bound from the tool in `a_paid_text_row_stops_for_the_approval_of_the_person`; unit `a_page_is_checked_against_the_key_terms_of_the_topic`, `key_terms_leave_out_small_words_articles_and_plurals` |
| 2, 3, Arabic | `when_no_page_is_about_the_topic_the_error_names_the_pages_and_the_two_ways_out` |
| 4 | `write_asks_one_more_time_for_a_number_and_a_card_and_never_passes_in_silence`; unit `a_script_with_number_facts_must_say_one_and_show_one`, `a_year_can_be_a_stat_card` |
| 5 | unit `two_publishers_confirm_with_other_words_other_unit_names_and_a_rounded_number`, `the_same_number_of_another_quantity_confirms_nothing`, `a_conflict_is_still_found`; end to end in the first test of this table (`450 nm` and `450 nanometres`) |

## 9. Not done

- No run with a real text model after this change. The search-terms call, the number rule of `write` and the new match of `confirmed` ran only against the fake text endpoint.
- With no model, a question still finds pages that only share its words. The title form gives the article of each key word, and the check takes out the worst of them, not all.
- The key terms have no stemmer: two rules for Arabic (the article, the consonants of a word) and one for English (the plural "s").
- `write` makes no `table` and no `number_line` card. A `number_line` has two end values that are in no fact, and gate `claims` reads them as claims with no source. A `table` needs a header row, and its words are in no fact: the code would have to write text on the picture that no page and no person gave. So the number rule is kept by `stat` and `bars` only.
- A short name inside a longer one confirms when the value is the same: "boiling point" (100 °C) and "boiling point of the mixture" (100 °C). The rule wants two shared terms and the same value; it cannot see that the longer name is another thing.
- The unit table has no conversion between units.
- Only the `wikipedia` row has a title form. Wikidata has an entity search that is near to one; it gets the queries as before.
- A page is compared with the pages before it by its whole text. Two addresses of one article with a small difference in the text are two pages.
