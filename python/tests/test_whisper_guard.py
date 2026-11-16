"""The local speech to text plugin survives an empty alignment (G004). No model needed."""

import importlib.util
from pathlib import Path

PLUGIN = Path(__file__).parents[2] / "plugins/whisper/scriptvox_whisper/__main__.py"


def load():
    spec = importlib.util.spec_from_file_location("whisper_plugin", PLUGIN)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Model:
    def __init__(self, fail):
        self.fail = fail

    def find_alignment(self, tokenizer, text_tokens, *args, **kwargs):
        if self.fail:
            raise IndexError("boolean index did not match")
        return [["word"] for _ in text_tokens]


def test_an_empty_alignment_gives_no_word_times_and_no_crash():
    guard = load().guard_alignment
    assert guard(Model(True)).find_alignment(None, [[1], [2]], None, 1) == [[], []]


def test_a_good_alignment_is_left_alone():
    guard = load().guard_alignment
    assert guard(Model(False)).find_alignment(None, [[1]], None, 1) == [["word"]]
