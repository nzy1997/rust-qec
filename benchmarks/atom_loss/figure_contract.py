"""Bind the reader-facing figures and methodology to validated data and source.

Checksums alone cannot reject an incorrectly generated but coherently resealed
figure. Regenerate in temporary storage, never overwrite the evidence being
checked. SVG is deterministic and compared in full; PNG compression is ignored
but pixels and metadata must match exactly. No visual similarity tolerance.
"""
import importlib.metadata
from pathlib import Path
import shutil
import tempfile

from .artifacts import FIGURE_INPUTS, FIGURE_NAMES


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
        if (root/svg).read_bytes() != (redrawn/svg).read_bytes():
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


def verify_presentation(root):
    methodology = Path(__file__).with_name('README.md')
    if (root/'methodology.md').read_bytes() != methodology.read_bytes():
        raise ValueError('Published methodology differs from source README')
    from .plot import render
    with tempfile.TemporaryDirectory(prefix='atom-loss-figure-check-') as temp:
        redrawn = Path(temp)
        for name in FIGURE_INPUTS:
            shutil.copyfile(root/name, redrawn/name)
        render(redrawn)
        compare_figures(root, redrawn)
