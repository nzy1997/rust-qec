"""Adversarial guard-policy regressions; no native programs are executed."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

import axis_preflight as policy


class AxisPreflightTests(unittest.TestCase):
    def test_named_guards_require_successful_complete_records_and_exact_summary(self):
        for index,names in enumerate(policy.NAMES):
            records=['test '+name+' ... ok' for name in reversed(names)]
            summary=f'test result: ok. {len(names)} passed; 0 failed; 0 ignored; 0 measured; 287 filtered out; finished in 0.93s'
            log='compiler warning\nrunning '+str(len(names))+' tests\n'+'\n'.join(records+[summary])+'\n'
            policy.check(index,log)
            invalid=[
                '\n'.join(records[:-1]+[summary]),
                '\n'.join(records+[records[0],summary]),
                log.replace(' ... ok',' ... ignored',1),
                log.replace(' ... ok',' ... FAILED',1),
                log.replace(names[0],'unrelated::'+names[0]),
                '\n'.join(records+[summary,summary]),
                log.replace(f'{len(names)} passed',f'{len(names)+1} passed'),
                log.replace('0 failed','1 failed'),
                log.replace('0 ignored','1 ignored'),
                log.replace('0 measured','1 measured'),
                log.replace(' ... ok',' ok',1),
                'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out\n',
            ]
            for bad in invalid:
                with self.subTest(index=index,log=bad),self.assertRaises(ValueError):policy.check(index,bad)
        for bad in [True,-1,4,'1']:
            with self.subTest(index=bad),self.assertRaises(ValueError):policy.check(bad,'')

    def test_exact_role_binding_rejects_missing_extra_changed_and_control_sources(self):
        heads=dict(baseline=policy.BASELINE,candidate=policy.CANDIDATE,control=policy.BASELINE)
        policy.require_heads(heads)
        for change in [dict(baseline='a'*40),dict(candidate='b'*40),dict(control='d'*40),dict(other=policy.BASELINE)]:
            with self.subTest(change=change),self.assertRaises(ValueError):policy.require_heads(dict(heads,**change))
        for name in heads:
            incomplete=dict(heads);incomplete.pop(name)
            with self.subTest(missing=name),self.assertRaises(ValueError):policy.require_heads(incomplete)

    def test_peer_rejects_wrong_source_before_cloning_or_building(self):
        import prepare
        with tempfile.TemporaryDirectory() as directory:
            out=Path(directory)/'never-created'
            for baseline,candidate in [(policy.BASELINE,'b'*40),('a'*40,policy.CANDIDATE)]:
                with self.subTest(baseline=baseline,candidate=candidate), \
                     mock.patch.object(sys,'argv',['prepare.py','--out',str(out),'--baseline-ref',baseline,'--rust-ref',candidate]), \
                     mock.patch.object(sys,'version_info',(3,12)), \
                     mock.patch.object(prepare.platform,'system',return_value='Linux'), \
                     mock.patch.object(prepare.platform,'machine',return_value='x86_64'), \
                     mock.patch.object(prepare,'invoke') as invoked, \
                     self.assertRaisesRegex(ValueError,'exact reviewed materialized-cache'):
                    prepare.main()
                invoked.assert_not_called();self.assertFalse(out.exists())

    def test_standalone_cdf_rejects_any_peer_profile_even_when_resealed(self):
        here=Path(__file__).resolve().parent.parent/'cdf_source_pair'
        spec=importlib.util.spec_from_file_location('standalone_cdf_fixture',here/'test_bundle_fixture.py')
        fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)/'bundle';verifier=fixture.build_bundle(root)
            verifier.verify_bundle(root)
            before={p.relative_to(root):p.read_bytes() for p in root.rglob('*') if p.is_file()}
            for profile in [policy.PROFILE,None,'materialized-default','unknown']:
                for name,data in before.items():(root/name).write_bytes(data)
                path=root/'preparation/preparation.json';data=json.loads(path.read_text());data['native_preflight_profile']=profile;path.write_text(json.dumps(data,separators=(',',':'))+'\n')
                (root/'preparation/seal.json').unlink();verifier.evidence.seal_preparation(root/'preparation','c'*40)
                (root/'original-seal.json').write_text(json.dumps(dict(files=verifier.files(root),scope='SYNTHETIC TEST ONLY'),separators=(',',':'))+'\n')
                with self.subTest(profile=profile),self.assertRaisesRegex(ValueError,'standalone CDF verifier rejects'):
                    verifier.verify_bundle(root)
