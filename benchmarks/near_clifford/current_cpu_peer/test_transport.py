"""Actual failed, timed-out and cancelled child lifecycle regressions."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
def load(name):
    spec=importlib.util.spec_from_file_location('transport_'+name,HERE/(name+'.py'))
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module
pilot=load('run');outer=load('run_retained');prepare=load('prepare')

class TransportContracts(unittest.TestCase):
    def test_failed_preparation_preserves_actual_receipt_and_streams(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            with self.assertRaisesRegex(ValueError,'actual receipt retained'):
                prepare.invoke(root,'failure',[sys.executable,'-I','-c','print("actual failure");raise SystemExit(17)'],root,os.environ.copy())
            receipt=json.loads((root/'failure.receipt.json').read_text())
            self.assertEqual(receipt['exit_code'],17)
            self.assertTrue(receipt['child_waited'])
            self.assertIsNone(receipt['cancellation'])
            self.assertEqual(receipt['stdout_sha256'],prepare.sha(root/'failure.stdout'))
            self.assertEqual((root/'failure.stdout').read_text(),'actual failure\n')
            self.assertEqual(subprocess.run(['ps','-p',str(receipt['child_pid']),'-o','pid='],capture_output=True).returncode,1)

    def test_valid_json_from_failed_or_timed_out_child_is_retained_and_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            recorder=pilot.Recorder(Path(name))
            for command,timeout in [('import sys;print(\'{"status":"ok"}\');sys.exit(17)',3),('import time;print(\'{"status":"ok"}\',flush=True);time.sleep(5)',.05)]:
                event,data=recorder.invoke([sys.executable,'-I','-c',command],'synthetic',timeout=timeout)
                self.assertEqual(data,{'status':'ok'});self.assertTrue(event['child_waited']);self.assertFalse(event['exit_code']==0 and not event['timed_out'])
                self.assertEqual(subprocess.run(['ps','-p',str(event['child_pid']),'-o','pid='],capture_output=True).returncode,1)

    def test_cancelled_producer_reaps_worker_in_separate_session(self):
        with tempfile.TemporaryDirectory() as name:
            output=Path(name);script=output/'producer.py'
            script.write_text("import importlib.util,signal,sys;from pathlib import Path\n"+"s=importlib.util.spec_from_file_location('p',"+repr(str(HERE/'run.py'))+");p=importlib.util.module_from_spec(s);s.loader.exec_module(p)\n"+"signal.signal(signal.SIGTERM,p.cancel);p.Recorder(Path("+repr(name)+")).invoke([sys.executable,'-I','-c','import time;time.sleep(30)'],'cancel')\n")
            with (output/'producer.log').open('wb') as log:
                child=subprocess.Popen([sys.executable,'-I',str(script)],stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
                try:
                    active=output/'active-child.json';deadline=time.monotonic()+10
                    while not active.exists() and time.monotonic()<deadline:time.sleep(.01)
                    self.assertTrue(active.exists());worker=json.loads(active.read_text())['child_pid']
                    cleanup=outer.stop_producer(child,active,grace=3)
                    self.assertNotEqual(child.returncode,0);self.assertFalse(active.exists())
                    event=json.loads((output/'events.jsonl').read_text());self.assertEqual(event['child_pid'],worker);self.assertTrue(event['child_waited']);self.assertEqual(event['exit_code'],-9);self.assertEqual(event['cancellation'],'CampaignCancelled');self.assertEqual(cleanup[0]['exit_code'],1)
                finally:
                    if child.poll() is None:os.killpg(child.pid,signal.SIGKILL);child.wait()

if __name__=='__main__':unittest.main()
