"""Regressions for resealed presentation corruption, including optimized Python."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import struct
import sys
import tempfile
import unittest
import zlib

from PIL import Image
from PIL.PngImagePlugin import PngInfo
import matplotlib.pyplot as plt

from .artifacts import FIGURE_INPUTS, FIGURE_NAMES
from .figure_contract import compare_figures, verify_presentation
from .plot import render
from .source_contract import ROOT


class FigureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.redrawn = Path(cls.temp.name)/'redrawn'
        cls.redrawn.mkdir()
        for name in FIGURE_INPUTS:
            shutil.copyfile(ROOT/'site/static/data/atom-loss'/name, cls.redrawn/name)
        shutil.copyfile(ROOT/'benchmarks/atom_loss/README.md', cls.redrawn/'methodology.md')
        render(cls.redrawn)

    def test_deterministic_redraw_ignores_ambient_style(self):
        with plt.rc_context({'font.size': 30, 'axes.facecolor': 'magenta',
                             'svg.hashsalt': 'ambient-random-id'}):
            verify_presentation(self.redrawn)

    def test_every_svg_and_png_is_checked(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)/'bundle'
            shutil.copytree(self.redrawn, root)
            for name in FIGURE_NAMES:
                for ext in ['svg', 'png']:
                    filename = name+'.'+ext
                    original = (root/filename).read_bytes()
                    # A different valid figure models an accidentally copied old chart.
                    other = next(n for n in FIGURE_NAMES if n != name)+'.'+ext
                    shutil.copyfile(root/other, root/filename)
                    with self.subTest(file=filename), self.assertRaisesRegex(ValueError, filename):
                        compare_figures(root, self.redrawn)
                    (root/filename).write_bytes(original)

    def test_modified_input_requires_different_figures(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)/'bundle'
            shutil.copytree(self.redrawn, root)
            path = root/'sampling.json'
            data = json.loads(path.read_text())
            for record in data[0]['rust']['records']:
                record['sample_seconds'] *= 1000
            path.write_text(json.dumps(data))
            with self.assertRaisesRegex(ValueError, 'sampling-throughput.svg'):
                verify_presentation(root)

    def test_trailing_png_metadata_is_checked_after_loading(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)/'bundle'
            shutil.copytree(self.redrawn, root)
            path = root/'accuracy-time.png'
            original = path.read_bytes()
            exif = Image.Exif()
            exif[274] = 3  # A viewer honoring this tag rotates the chart 180 degrees.
            for tag, data in [(b'eXIf', exif.tobytes()[6:]),
                              (b'tEXt', b'Description\x00A false claim about the chart')]:
                chunk = (struct.pack('>I', len(data))+tag+data
                         + struct.pack('>I', zlib.crc32(tag+data) & 0xffffffff))
                path.write_bytes(original[:-12]+chunk+original[-12:])
                with self.subTest(tag=tag), self.assertRaisesRegex(ValueError, 'accuracy-time.png'):
                    compare_figures(root, self.redrawn)

    def test_png_with_corrupted_chunk_crc_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)/'bundle'
            shutil.copytree(self.redrawn, root)
            path = root/'accuracy-time.png'
            content = bytearray(path.read_bytes())
            offset = 8
            while content[offset+4:offset+8] != b'IDAT':
                offset += 12+struct.unpack('>I', content[offset:offset+4])[0]
            crc = offset+8+struct.unpack('>I', content[offset:offset+4])[0]
            content[crc] ^= 1
            path.write_bytes(content)
            with self.assertRaisesRegex(ValueError, 'Invalid published PNG: accuracy-time.png'):
                compare_figures(root, self.redrawn)

    def test_resealed_corruption_rejected_by_full_verifier_in_both_modes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)/'bundle'
            source = ROOT/'site/static/data/atom-loss'
            shutil.copytree(source, root)
            for defect in ['fabricated-svg', 'changed-pixel', 'swapped-png', 'methodology']:
                name = {'fabricated-svg': 'accuracy-time.svg', 'changed-pixel': 'accuracy-time.png',
                        'swapped-png': 'sampling-throughput.png', 'methodology': 'methodology.md'}[defect]
                if defect == 'fabricated-svg':
                    (root/name).write_text('<svg xmlns="http://www.w3.org/2000/svg">'
                                          '<text>RustQEC is 1000x faster</text></svg>')
                elif defect == 'changed-pixel':
                    with Image.open(root/name) as image:
                        image.load()
                        changed = image.copy()
                        metadata = PngInfo()
                        metadata.add_text('Software', image.info['Software'])
                        dpi = image.info['dpi']
                    pixel = changed.getpixel((0, 0))
                    changed.putpixel((0, 0), (pixel[0] ^ 255, *pixel[1:]))
                    changed.save(root/name, pnginfo=metadata, dpi=dpi)
                elif defect == 'swapped-png':
                    shutil.copyfile(root/'logical-error-rate.png', root/name)
                else:
                    (root/name).write_text('An unreviewed claim of universal speedup.')
                manifest = json.loads((source/'bundle.json').read_text())
                manifest['sha256'][name] = hashlib.sha256((root/name).read_bytes()).hexdigest()
                (root/'bundle.json').write_text(json.dumps(manifest))
                for flags in [[], ['-O']]:
                    with self.subTest(defect=defect, flags=flags):
                        result = subprocess.run([sys.executable, *flags, '-m',
                                                 'benchmarks.atom_loss.verify', str(root)],
                                                cwd=ROOT, capture_output=True, text=True)
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn('ValueError', result.stderr)
                        self.assertIn('methodology' if defect == 'methodology' else name, result.stderr)
                shutil.copyfile(source/name, root/name)
