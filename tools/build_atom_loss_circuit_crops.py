#!/usr/bin/env python3
"""Crop two loss locations from the tutorial's real rstim circuit SVG."""
import argparse
import copy
from html.parser import HTMLParser
import os
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET

REPO = Path(__file__).resolve().parents[1]
SVG = 'http://www.w3.org/2000/svg'
ET.register_namespace('', SVG)


class ModelCommand(HTMLParser):
    def __init__(self):
        super().__init__()
        self.active = False
        self.command = ''

    def handle_starttag(self, tag, attrs):
        if tag == 'pre':
            self.active = dict(attrs).get('data-atom-loss-step') == 'model'

    def handle_endtag(self, tag):
        if tag == 'pre':
            self.active = False

    def handle_data(self, text):
        if self.active:
            self.command += text


def bounds(node):
    tag = node.tag.rsplit('}', 1)[-1]
    value = lambda key: float(node.get(key, '0'))
    if tag == 'line':
        return (min(value('x1'), value('x2')), min(value('y1'), value('y2')),
                max(value('x1'), value('x2')), max(value('y1'), value('y2')))
    if tag == 'rect':
        return (value('x'), value('y'), value('x') + value('width'), value('y') + value('height'))
    if tag == 'circle':
        return (value('cx') - value('r'), value('cy') - value('r'),
                value('cx') + value('r'), value('cy') + value('r'))
    if tag == 'text':
        # The viewBox performs exact clipping; generous text bounds preserve
        # the renderer's probability and measurement-record annotations.
        return (value('x') - 64, value('y') - 16, value('x') + 64, value('y') + 16)
    raise ValueError(f'Unexpected native SVG primitive: {tag}')


def crop(source, filename, start, y, title, destination, width=272):
    left, top, height = start - 64, y - 38, 88
    root = ET.Element(f'{{{SVG}}}svg', {
        'width': str(width * 2), 'height': '176',
        'viewBox': f'{left:g} {top:g} {width} {height}',
        'role': 'img', 'aria-labelledby': 'title',
    })
    ET.SubElement(root, f'{{{SVG}}}title', {'id': 'title'}).text = title
    ET.SubElement(root, f'{{{SVG}}}desc').text = (
        'Cropped from rstim render_svg of the exact circuit generation command '
        'on the atom-loss tutorial. The q2 label is repeated at the crop edge. '
        'This is the circuit definition, without sampled events.'
    )
    ET.SubElement(root, f'{{{SVG}}}rect', {
        'x': f'{left:g}', 'y': f'{top:g}', 'width': str(width), 'height': str(height), 'fill': '#fff',
    })
    native = source.find(f'{{{SVG}}}g')
    group = ET.SubElement(root, f'{{{SVG}}}g', native.attrib)
    for node in native:
        if node.tag == f'{{{SVG}}}text' and not left <= float(node.get('x', '0')) <= left + width:
            # An off-screen detector's long caption can extend backwards into
            # the readout crop; keep only labels whose own anchors are visible.
            continue
        x0, y0, x1, y1 = bounds(node)
        if x1 >= left and x0 <= left + width and y1 >= top and y0 <= top + height:
            group.append(copy.deepcopy(node))
    # Keep the renderer's own wire label visible after cropping horizontally.
    ET.SubElement(group, f'{{{SVG}}}rect', {
        'x': f'{left:g}', 'y': f'{y - 14:g}', 'width': '32', 'height': '28', 'fill': '#fff',
    })
    label = copy.deepcopy(next(node for node in native if node.tag == f'{{{SVG}}}text' and node.text == 'q2'))
    label.set('x', f'{left + 8:g}')
    group.append(label)
    ET.indent(root)
    ET.ElementTree(root).write(destination / filename, encoding='unicode', xml_declaration=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    target = Path(os.environ.get('CARGO_TARGET_DIR', REPO / 'target'))
    if not target.is_absolute():
        target = REPO / target
    parser.add_argument('--binary', type=Path, default=target / 'debug/rstim')
    args = parser.parse_args()
    binary = args.binary.resolve()
    model = ModelCommand()
    model.feed((REPO / 'site/templates/atom-loss.html').read_text())
    if not model.command.strip():
        raise ValueError('Missing tutorial circuit generation command')
    destination = REPO / 'site/static/examples/atom-loss'
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='rstim-loss-crops-') as temporary:
        env = dict(os.environ, PATH=str(binary.parent) + os.pathsep + os.environ['PATH'])
        subprocess.run(['sh', '-eu'], input=model.command, text=True, cwd=temporary, env=env, check=True, capture_output=True)
        rendered = Path(temporary) / 'circuit.svg'
        subprocess.run([str(binary), 'render_svg', '--in', str(Path(temporary) / 'circuit.stim'), '--out', str(rendered)], check=True)
        source = ET.parse(rendered).getroot()
        texts = list(source.iter(f'{{{SVG}}}text'))
        y = float(next(node for node in texts if node.text == 'q2').get('y'))
        on_wire = lambda label: [node for node in texts if node.text == label and float(node.get('y', '0')) == y]
        operation = float(on_wire('H')[0].get('x'))
        readout = float(on_wire('ML')[-1].get('x'))
        measurement = max(float(node.get('x')) for node in on_wire('LOSS') if float(node.get('x')) < readout)
        for x, probability in ((operation + 144, '0.002'), (measurement, '0.003')):
            assert any(node.get('class') == 'param-note' and float(node.get('x', '0')) == x and float(node.get('y', '0')) == y + 28 and node.text == probability for node in texts), 'Tutorial loss location changed'
        crop(source, 'operation-loss-crop.svg', operation, y, 'Operation loss: H, Pauli depolarization, then LOSS(0.002) on q2', destination)
        crop(source, 'measurement-loss-crop.svg', measurement, y, 'Measurement loss: LOSS(0.003) before final ML on q2', destination, width=164)
    print('Rendered operation-loss and measurement-loss crops from the tutorial circuit')


if __name__ == '__main__':
    main()
