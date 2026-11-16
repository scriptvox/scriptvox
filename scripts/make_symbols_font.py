"""Builds assets/fonts/RawivoxSymbols-Regular.ttf from Noto Sans Math (OFL-1.1).

Noto Sans Math has tall OS/2 win metrics (5655 units for a 1000 em). Libass sizes a fallback glyph by those
metrics, so a symbol that Noto Sans lacks (an arrow, "≤", "≈", "√") showed at about a quarter of the caption
size. This keeps only the symbols (U+2000 to U+2BFF) that Noto Sans has not, sets Noto Sans's win metrics, and
renames the family (OFL: a modified font gets its own name). Run: python3 scripts/make_symbols_font.py (needs fontTools)
"""

from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

fonts = Path(__file__).resolve().parent.parent / "assets" / "fonts"
sans = TTFont(fonts / "NotoSans-Regular.ttf")
have = set(sans.getBestCmap())
math = TTFont(fonts / "NotoSansMath-Regular.ttf")
keep = sorted(c for c in math.getBestCmap() if 0x2000 <= c <= 0x2BFF and c not in have)

opts = subset.Options()
opts.drop_tables += ["MATH"]
opts.name_IDs = ["*"]
sub = subset.Subsetter(opts)
sub.populate(unicodes=keep)
sub.subset(math)

os2, s2 = math["OS/2"], sans["OS/2"]
os2.usWinAscent, os2.usWinDescent = s2.usWinAscent, s2.usWinDescent
math["hhea"].ascent, math["hhea"].descent = sans["hhea"].ascent, sans["hhea"].descent
os2.sTypoAscender, os2.sTypoDescender = s2.sTypoAscender, s2.sTypoDescender
os2.sTypoLineGap = s2.sTypoLineGap
name = math["name"]
for rec in list(name.names):
    if rec.nameID in (1, 3, 4, 6, 16, 17, 21, 22):
        name.removeNames(nameID=rec.nameID)
for nid, text in [(1, "Rawivox Symbols"), (2, "Regular"), (3, "Rawivox Symbols Regular"),
                  (4, "Rawivox Symbols"), (6, "RawivoxSymbols-Regular")]:
    name.setName(text, nid, 3, 1, 0x409)
math.save(fonts / "RawivoxSymbols-Regular.ttf")
print(len(keep), "symbols")
