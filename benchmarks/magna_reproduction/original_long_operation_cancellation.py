#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Observe the unchanged original SIGALRM test without claiming Rust signal ABI."""
import argparse
from fractions import Fraction
import json
from pathlib import Path
import signal
import sys
import time
import unittest

import reference
from witness_environment import native_reference
from witness_paths import admit_destinations

METHOD = "test_file_api.FileApiTests.test_signal_handlers_run_during_one_long_native_batch"


def capture(upstream):
    if not hasattr(signal,"SIGALRM") or not hasattr(signal,"setitimer"):
        raise ValueError("original signal capture requires POSIX SIGALRM and setitimer")
    if signal.getitimer(signal.ITIMER_REAL) != (0,0):
        raise ValueError("capture requires a fresh process without an active real timer")
    previous = signal.getsignal(signal.SIGALRM)
    requests = []
    original_signal, original_timer = signal.signal, signal.setitimer
    def set_handler(number,handler):
        result = original_signal(number,handler)
        requests.append(dict(kind="handler",signal=int(number),callable=callable(handler),
                             restores_previous=handler is previous))
        return result
    def set_timer(which,delay,interval=0):
        result = original_timer(which,delay,interval)
        requests.append(dict(kind="timer",which=int(which),delay=str(Fraction(str(delay))),
                             interval=str(Fraction(str(interval)))))
        return result
    with native_reference(upstream) as cap:
        sys.path[:0] = [str(upstream),str(Path(upstream)/"tests/core")]
        signal.signal, signal.setitimer = set_handler, set_timer
        started=time.monotonic()
        try:
            result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromNames([METHOD]))
        finally:
            signal.signal, signal.setitimer = original_signal, original_timer
            restored=signal.getsignal(signal.SIGALRM) is previous
            disabled=signal.getitimer(signal.ITIMER_REAL) == (0,0)
            # Clean up before reporting a failing original method as failure.
            if not disabled:original_timer(signal.ITIMER_REAL,0)
            if not restored:original_signal(signal.SIGALRM,previous)
        if not result.wasSuccessful() or result.testsRun!=1 or result.skipped:
            raise ValueError("complete original signal method did not pass")
        expected=[dict(kind="handler",signal=int(signal.SIGALRM),callable=True,restores_previous=False),
            dict(kind="timer",which=int(signal.ITIMER_REAL),delay="1/5",interval="0"),
            dict(kind="timer",which=int(signal.ITIMER_REAL),delay="0",interval="0"),
            dict(kind="handler",signal=int(signal.SIGALRM),callable=callable(previous),restores_previous=True)]
        if requests!=expected or not restored or not disabled:
            raise ValueError("original signal/timer restoration protocol changed")
        return dict(schema="rmagna.original-long-cancellation.v1",upstream_commit=cap.manifest["upstream"]["commit"],
            reference_sha256=cap.manifest_sha256,native_frontend_sha256=cap.native_sha256,
            producer_sha256=reference.digest(Path(__file__)),
            source_sha256={name:cap.manifest["upstream"]["files"][name] for name in
                ("tests/core/test_file_api.py","core/magna/file_api.py")},
            methods=[METHOD],original_methods_passed=1,original_shots=10_000_000,original_progress_every=10_000_000,
            original_alarm_seconds="1/5",original_exception="Stop",requests=requests,
            signal_handler_restored=restored,timer_disabled=disabled,seconds=time.monotonic()-started,
            scope="unchanged_original_signal_contract_not_Rust_Python_ABI")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream",type=Path,required=True)
    parser.add_argument("--out",type=Path,required=True)
    args=parser.parse_args()
    try:
        destination,=admit_destinations([args.out],args.upstream,protected_roots=[reference.HERE])
        data=capture(args.upstream)
        with destination.open("x",encoding="utf-8") as stream:
            json.dump(data,stream,separators=(",",":"),allow_nan=False);stream.write("\n")
    except (ValueError,OSError) as error:
        parser.error(str(error))
    print("all1 original signal method passed;actual handler/timer protocol restored;Python ABI remains separate")


if __name__=="__main__":main()
