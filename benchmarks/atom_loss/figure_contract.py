"""Bind the reader-facing figures and methodology to validated data and source.

Checksums alone cannot reject an incorrectly generated but coherently resealed
figure. Regenerate in temporary storage, never overwrite the evidence being
checked. SVG permits only one serialization unit of absolute roundoff in anonymous path
coordinates and marker-use positions; all other bytes are exact. PNG compression is ignored but pixels
and metadata must match exactly. No perceptual image similarity tolerance.
"""
from decimal import Decimal
import importlib.metadata
from pathlib import Path
import re
import shutil
import tempfile

from .artifacts import FIGURE_INPUTS, FIGURE_NAMES


def same_svg(actual, expected):
    """Matplotlib writes six decimal places; platform libm can round a tie apart.

    Only anonymous path d coordinates and marker-use x/y positions may differ
    by <= 0.000001 pt, absolutely.
    Commands, separators, number counts and every byte outside these coordinates
    remain exact. In particular no tolerance applies to transforms, text, style,
    viewBox, IDs/references or font glyph definitions (which have an id first).
    Marker references use Matplotlib's m-prefixed ten-hex-digit IDs; text glyph
    uses retain exact positions. PNG pixels are checked separately and exactly.
    """
    if actual == expected:
        return True
    number = re.compile(rb'[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?')
    def split(data):
        coordinates = []
        def path(match):
            structure = number.sub(b'#', match[1])
            if not set(re.findall(rb'[A-Za-z]', structure)) <= {b'M', b'L', b'Q', b'C', b'Z', b'z'}:
                # No tolerance for relative-coordinate accumulation or arc flags.
                return match[0]
            values = number.findall(match[1])
            coordinates.extend(Decimal(value.decode('ascii')) for value in values)
            return b'<path d="'+structure+b'"'
        structure = re.sub(rb'<path d="([^"]*)"', path, data)
        # Error-bar caps are <use> markers rather than anonymous paths. Their
        # endpoints can round the same libm tie apart as the connecting path.
        # Restrict this to canonical marker references and explicit x/y attrs;
        # transforms, glyph positions, styles and every other byte remain exact.
        marker = re.compile(
            rb'(<use xlink:href="#m[0-9a-f]{10}" x=")(' + number.pattern
            + rb')(" y=")(' + number.pattern + rb')(")'
        )
        def position(match):
            coordinates.extend(Decimal(match[i].decode('ascii')) for i in [2, 4])
            return match[1] + b'#' + match[3] + b'#' + match[5]
        return marker.sub(position, structure), coordinates
    actual_structure, a = split(actual)
    expected_structure, b = split(expected)
    return (actual_structure == expected_structure and len(a) == len(b)
            and all(abs(x-y) <= Decimal('0.000001') for x, y in zip(a, b)))


def require_renderer():
    from matplotlib import ft2font
    requirements = Path(__file__).with_name('requirements.txt').read_text().splitlines()
    for line in requirements:
        if line.split('==')[0] in {'matplotlib', 'numpy', 'pillow', 'contourpy', 'cycler',
                                  'fonttools', 'kiwisolver', 'packaging', 'pyparsing',
                                  'python-dateutil', 'six'}:
            package, version = line.split('==')
            if importlib.metadata.version(package) != version:
                raise ValueError('Install the pinned figure renderer: '+line)
    if ft2font.__freetype_version__ != '2.6.1':
        raise ValueError('Figure verification requires the Matplotlib wheel with bundled FreeType 2.6.1')


def compare_figures(root, redrawn):
    from PIL import Image
    for name in FIGURE_NAMES:
        svg = name+'.svg'
        if not same_svg((root/svg).read_bytes(), (redrawn/svg).read_bytes()):
            raise ValueError('Figure differs from validated JSON: '+svg)
        png = name+'.png'
        try:
            with Image.open(root/png) as encoded:
                encoded.verify()  # Check chunk CRCs as well as decoded content.
        except (OSError, SyntaxError, ValueError) as error:
            raise ValueError('Invalid published PNG: '+png) from error
        with Image.open(root/png) as actual, Image.open(redrawn/png) as expected:
            # PNG permits metadata after IDAT; Pillow discovers it during load.
            # In particular trailing EXIF orientation can change displayed pixels.
            actual.load()
            expected.load()
            if (actual.format != 'PNG' or actual.n_frames != 1
                    or actual.mode != expected.mode or actual.size != expected.size
                    or actual.info != expected.info or actual.tobytes() != expected.tobytes()):
                raise ValueError('Figure differs from validated JSON: '+png)


def verify_presentation(root, *, methodology=None):
    if methodology is None:
        methodology = Path(__file__).with_name('README.md').read_bytes()
    if (root/'methodology.md').read_bytes() != methodology:
        raise ValueError('Published methodology differs from source README')
    from .plot import render
    with tempfile.TemporaryDirectory(prefix='atom-loss-figure-check-') as temp:
        redrawn = Path(temp)
        for name in FIGURE_INPUTS:
            shutil.copyfile(root/name, redrawn/name)
        render(redrawn)
        compare_figures(root, redrawn)
