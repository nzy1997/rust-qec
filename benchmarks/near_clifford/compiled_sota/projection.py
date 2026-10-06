"""Native raw-record projection for the frozen linear corpus; no gate lowering."""
import re


def physical_width(text):
    """Count target indices for the frozen linear Stim corpus (no REPEAT)."""
    targets = []
    for line in text.splitlines():
        words=line.split('#',1)[0].split()
        if words and words[0] == 'REPEAT':
            raise ValueError('REPEAT unsupported in the frozen corpus counter')
        for word in words[1:]:
            for factor in word.split('*'):
                match=re.fullmatch(r'!?[XYZ]?(\d+)',factor)
                if match: targets.append(int(match[1]))
    return max(targets,default=-1)+1


def records_only(text):
    """Project the output contract onto all raw records, without postselection."""
    return '\n'.join(line for line in text.splitlines()
        if not (words := line.split('#', 1)[0].split()) or
        words[0].split('(', 1)[0] not in ('DETECTOR', 'OBSERVABLE_INCLUDE')) + '\n'
