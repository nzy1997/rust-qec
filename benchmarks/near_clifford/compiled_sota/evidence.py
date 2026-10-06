"""Lossless compact record transcripts; no NumPy dependency for verification."""
import base64
import zlib


def compact(payload):
    bits = payload['measurements']
    if len(bits) != payload['shots']*payload['width'] or any(type(b) is not int or b not in (0,1) for b in bits):
        raise ValueError('invalid raw record transcript')
    packed = bytearray((len(bits)+7)//8)
    for i, bit in enumerate(bits):
        packed[i//8] |= bit << (7-i%8)
    result = dict(payload)
    result['measurements'] = {'codec':'zlib-packbits-big-v1',
        'data':base64.b64encode(zlib.compress(bytes(packed),9)).decode()}
    return result


def expand(payload):
    encoded = payload['measurements']
    if encoded['codec'] != 'zlib-packbits-big-v1':
        raise ValueError('unknown transcript codec')
    total = payload['shots']*payload['width']
    if type(total) is not int or total < 1 or total > 10_000_000:
        raise ValueError('invalid transcript dimensions')
    decoder = zlib.decompressobj()
    packed = decoder.decompress(base64.b64decode(encoded['data'],validate=True), (total+7)//8+1)
    if len(packed) != (total+7)//8 or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:
        raise ValueError('invalid compressed transcript')
    if total%8 and packed[-1] & ((1 << (8-total%8))-1):
        raise ValueError('nonzero transcript padding')
    result = dict(payload)
    result['measurements'] = [(packed[i//8] >> (7-i%8)) & 1 for i in range(total)]
    return result


def parity_counts(payload, masks):
    bits = payload['measurements']
    width, shots = payload['width'], payload['shots']
    if len(bits)!=width*shots or any(type(b) is not int or b not in (0,1) for b in bits):
        raise ValueError('invalid transcript bits')
    counts = [0]*len(masks)
    bitmasks = [sum(1 << q for q in mask) for mask in masks]
    for offset in range(0,len(bits),width):
        row = sum(bit << q for q,bit in enumerate(bits[offset:offset+width]))
        for i, mask in enumerate(bitmasks):
            counts[i] += (row & mask).bit_count()%2
    return counts
