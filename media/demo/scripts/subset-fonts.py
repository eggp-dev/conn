"""Rebuild bundled font subsets after changing copy. Requires fonttools + brotli."""
import argparse
from pathlib import Path
from fontTools import subset

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--regular", default="/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc")
parser.add_argument("--bold", default="/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc")
args = parser.parse_args()
characters = "".join(p.read_text() for p in (root / "src").rglob("*") if p.is_file())
characters += "".join(chr(code) for code in range(32, 127))
output = root / "public" / "fonts"
(output / "characters.txt").write_text(characters)
for weight, source in [("Regular", args.regular), ("Bold", args.bold)]:
    subset.main([source, "--font-number=1", f"--text-file={output / 'characters.txt'}", "--flavor=woff2", f"--output-file={output / f'ConnNoto-{weight}.woff2'}"])
