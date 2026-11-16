"""Independent BH code-weight oracle derived from the literal encoding prefix."""
from collections import Counter
from fractions import Fraction
from math import comb


def matrix(k, circuit):
    if type(k) is not int or k < 2 or k % 2:
        raise ValueError('BH outputs must be a positive even integer')
    n = 3*k+8
    lines = circuit.strip().splitlines()
    if not lines or lines[0].split() != ['H'] + [str(q) for q in range(k+3)]:
        raise ValueError('BH encoding prefix has an unexpected plus preparation')
    wires = [1 << q if q < k+3 else 0 for q in range(n+k)]
    end = False
    for line in lines[1:]:
        words = line.split()
        if not words:
            continue
        if words == ['H', str(n+k)]:
            end = True
            break
        if words[0] in {'S', 'S_DAG', 'Z', 'CZ'}:
            if len(words) < 2 or (words[0] == 'CZ' and len(words[1:]) % 2):
                raise ValueError('BH diagonal prefix has incomplete targets')
            if any(not 0 <= int(q) < n+k for q in words[1:]):
                raise ValueError('BH diagonal prefix index outside encoding')
        elif words[0] == 'CX' and len(words) > 1 and len(words[1:]) % 2 == 0:
            for a, b in zip(words[1::2], words[2::2]):
                a, b = int(a), int(b)
                if not 0 <= a < n+k or not 0 <= b < n+k or a == b:
                    raise ValueError('BH encoding CX outside original wires')
                wires[b] ^= wires[a]
        else:
            raise ValueError('unsupported BH encoding prefix operation: '+line)
    if not end:
        raise ValueError('BH resource injection boundary missing')
    rows = [sum(((column >> q) & 1) << j for j, column in enumerate(wires[:n]))
            for q in range(k+3)]
    validate_rows(k, rows)
    return rows


def validate_rows(k, rows):
    n = 3*k+8
    if (type(k) is not int or k < 2 or k % 2 or len(rows) != k+3
            or any(type(row) is not int or row < 0 or row.bit_length() > n for row in rows)):
        raise ValueError('invalid BH code matrix dimensions')
    basis = {}
    for row in rows:
        while row:
            pivot = row.bit_length()-1
            if pivot not in basis:
                basis[pivot] = row
                break
            row ^= basis[pivot]
    if len(basis) != len(rows):
        raise ValueError('BH code matrix is not full row rank')
    if [row.bit_count() % 2 for row in rows] != [1]*k+[0]*3:
        raise ValueError('BH odd/even row partition changed')
    for i, a in enumerate(rows):
        for j, b in enumerate(rows[i+1:], i+1):
            if (a & b).bit_count() % 2:
                raise ValueError('BH rows are not pairwise orthogonal')
            for c in rows[j+1:]:
                if (a & b & c).bit_count() % 2:
                    raise ValueError('BH rows are not triorthogonal')
    # Prove invariance under every permutation of the k odd generators from
    # the multiset of columns. This justifies binomial rather than 2^k sums.
    all_logical = (1 << k)-1
    columns = Counter(sum(((row >> j) & 1) << q for q, row in enumerate(rows))
                      for j in range(n))
    for column, multiplicity in columns.items():
        logical = column & all_logical
        if logical.bit_count() not in {0, 1, k-1, k}:
            raise ValueError('BH column is outside the symmetric orbit family')
        if logical in {0, all_logical}:
            continue
        for left in range(k):
            for right in range(left+1, k):
                swapped = column
                if ((column >> left) ^ (column >> right)) & 1:
                    swapped ^= (1 << left) | (1 << right)
                if columns[swapped] != multiplicity:
                    raise ValueError('BH columns do not have full logical permutation symmetry')


def exact(k, circuit, p):
    return exact_from_rows(k, matrix(k, circuit), p)


def exact_from_rows(k, rows, p):
    validate_rows(k, rows)
    p = Fraction(p)
    if not 0 <= p <= 1:
        raise ValueError('BH physical rate outside [0,1]')
    n = 3*k+8
    x = 1-2*p
    powers = [x**w for w in range(n+1)]
    even_words = [0]
    for row in rows[k:]:
        even_words += [word ^ row for word in even_words]
    acceptance = sum(powers[word.bit_count()] for word in even_words) / 8
    good = Fraction(0)
    logical = 0
    for weight in range(k+1):
        good += comb(k, weight)*sum(powers[(word ^ logical).bit_count()] for word in even_words)
        if weight < k:
            logical ^= rows[weight]
    good /= 1 << (k+3)
    return dict(acceptance=acceptance, joint_success=good,
                accepted_failure=acceptance-good,
                conditional_error=(acceptance-good)/acceptance if acceptance else None)

