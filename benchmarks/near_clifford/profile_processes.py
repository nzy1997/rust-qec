"""Profile-owned process receipts with cancellation-safe spawn/reap boundaries."""
import json
import os
from pathlib import Path
import signal
import subprocess
import time

from evidence_common import Recorder as OriginalRecorder, meta


class Recorder(OriginalRecorder):
    def __init__(self, root, output, source):
        super().__init__(root, output, source)
        (output / 'executed-process-recorder.py').write_bytes(Path(__file__).read_bytes())
        self.waited = []

    def run(self, label, argv, timeout=300):
        start = time.time()
        child = None
        cancellation = None
        kill_error = None
        with (self.output / (label + '.stdout')).open('xb') as out, (self.output / (label + '.stderr')).open('xb') as err:
            try:
                previous = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGTERM, signal.SIGINT})
                try:
                    child = subprocess.Popen(argv, cwd=self.root, stdout=out, stderr=err, start_new_session=True,
                        preexec_fn=lambda: signal.pthread_sigmask(signal.SIG_SETMASK, previous))
                    self.children.append(child.pid)
                    print(label, 'START', os.getpid(), child.pid, flush=True)
                    (self.output / (label + '.start.json')).write_text(json.dumps(
                        dict(controller_pid=os.getpid(), child_pid=child.pid, argv=argv, started=start), indent=2) + '\n')
                finally:
                    signal.pthread_sigmask(signal.SIG_SETMASK, previous)
                code = child.wait(timeout=timeout)
            except BaseException as exc:
                cancellation = type(exc).__name__
                if child is not None:
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    except PermissionError as denied:
                        # Privileged perf has its own150s GNU timeout watchdog.
                        # Record denied group cleanup and still wait the actual
                        # child; never label an unreaped child as closed.
                        kill_error = repr(denied)
                    code = child.wait(timeout=180 if kill_error else 20)
                else:
                    code = None
            if child is not None and child.returncode is not None:
                self.waited.append(child.pid)
        receipt = dict(controller_pid=os.getpid(), child_pid=child.pid if child else None,
                       child_waited=child is not None and child.returncode is not None,
                       argv=argv, started=start, closed=time.time(), exit_code=code,
                       cancellation=cancellation, kill_error=kill_error,
                       stdout=meta(self.output / (label + '.stdout')), stderr=meta(self.output / (label + '.stderr')))
        (self.output / (label + '.receipt.json')).write_text(json.dumps(receipt, indent=2) + '\n')
        print(label, 'CLOSED', code, cancellation, flush=True)
        return receipt

    def close(self, failure, context):
        reaped = self.children == self.waited
        if not reaped and failure is None:
            failure = 'actual child closure incomplete'
        (self.output / 'controller-closed.json').write_text(json.dumps(dict(
            controller_pid=os.getpid(), children=self.children, waited_children=self.waited,
            all_children_reaped=reaped, failure=failure, closed=time.time(), context=context), indent=2) + '\n')
        files = {p.relative_to(self.output).as_posix(): meta(p) for p in self.output.rglob('*') if p.is_file()}
        (self.output / 'seal.json').write_text(json.dumps(dict(files=files), indent=2) + '\n')
        print('SEALED', str(self.output), os.getpid(), self.children, len(files),
              meta(self.output / 'seal.json')['sha256'], 'failure', failure, flush=True)
