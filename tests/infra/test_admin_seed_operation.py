"""Execute the real operator script with an isolated Docker boundary, never production."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

class AdminSeedOperation(unittest.TestCase):
    def run_operation(self, email='synthetic@example.invalid', extra=()):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            sudo = directory / 'sudo'
            sudo.write_text('''#!/usr/bin/env python3
import os,sys
args=sys.argv[1:]
preserved=set()
if args and args[0].startswith('--preserve-env='):
    preserved=set(args.pop(0).split('=',1)[1].split(','))
env=dict(os.environ)
for key in ['ADMIN_EMAIL','ADMIN_NAME']:
    if key not in preserved: env.pop(key,None)
os.execvpe(args[0],args,env)
''')
            docker = directory / 'docker'
            docker.write_text('''#!/usr/bin/env python3
import os,json,sys
args=sys.argv[1:]
expected=['compose','--env-file','/private/synthetic-compose.env','-f',os.environ['EXPECTED_COMPOSE'],'exec','-T','-e','ADMIN_EMAIL','-e','ADMIN_NAME','backend','/usr/local/bin/imt-entrypoint','seed-admin']
if args!=expected: sys.exit(91)
with open(os.environ['MOCK_CAPTURE'],'w') as f: json.dump({'args':args,'stdin':sys.stdin.read(),'email':os.getenv('ADMIN_EMAIL'),'name':os.getenv('ADMIN_NAME')},f)
print('admin created (synthetic boundary)')
''')
            sudo.chmod(0o700)
            docker.chmod(0o700)
            env={**os.environ,'PATH':str(directory)+os.pathsep+os.environ['PATH'],
                 'IMT_ENV_FILE':'/private/synthetic-compose.env','ADMIN_NAME':'Synthetic operator',
                 'MOCK_CAPTURE':str(directory/'received.json'),
                 'EXPECTED_COMPOSE':str(ROOT/'deploy/compose.production.yaml')}
            env.pop('ADMIN_EMAIL',None)
            if email is not None: env['ADMIN_EMAIL']=email
            result=subprocess.run(['sh',str(ROOT/'deploy/operations.sh'),'seed-admin',*extra],env=env,input='synthetic-private-stdin-password',text=True,capture_output=True)
            captured=json.loads((directory/'received.json').read_text()) if (directory/'received.json').exists() else None
            return result,captured

    def test_seed_uses_existing_backend_entrypoint_and_private_stdin(self):
        result,captured=self.run_operation()
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(captured['stdin'],'synthetic-private-stdin-password')
        self.assertEqual(captured['email'],'synthetic@example.invalid')
        self.assertEqual(captured['name'],'Synthetic operator')
        self.assertNotIn('synthetic-private-stdin-password',str(captured['args']))
        self.assertNotIn('synthetic-private-stdin-password',result.stdout+result.stderr)

    def test_missing_email_never_calls_docker(self):
        result,captured=self.run_operation(email=None)
        self.assertNotEqual(result.returncode,0)
        self.assertIsNone(captured)

    def test_password_arguments_are_rejected_before_docker(self):
        result,captured=self.run_operation(extra=('not-a-password-argument',))
        self.assertNotEqual(result.returncode,0)
        self.assertIsNone(captured)
        self.assertNotIn('not-a-password-argument',result.stdout+result.stderr)

if __name__=='__main__': unittest.main()
