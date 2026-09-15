import json
from pathlib import Path

public = json.loads(Path('data/public/manifest.json').read_text())
private = json.loads(Path('data/private/manifest.json').read_text())
stats = json.loads(Path('decode-stats.json').read_text())
predictions = Path('predictions.b8').read_bytes()
answers = Path('data/private/answers.b8').read_bytes()
assert public['dataset_id'] == private['dataset_id'], 'Mismatched datasets'
assert public['circuit']['observables'] == 1, 'This check expects one observable'
assert len(predictions) == len(answers) == public['shots'] == stats['shot_count'], 'Incomplete rows'
assert all(bit in (0, 1) for bit in predictions + answers), 'Invalid padding bits'
errors = sum(predicted != answer for predicted, answer in zip(predictions, answers))
print(f"Decoded shots: {stats['shot_count']}")
print(f"Loss patterns: {stats['distinct_loss_patterns']}")
print(f"Logical errors: {errors} / {public['shots']}")
