#!/usr/bin/env python3
"""Owned smoke-only proof of the production entrypoint/seed path. Never prints credentials."""
import argparse
from http.cookies import SimpleCookie
import json
import os
from pathlib import Path
import secrets
import subprocess
import urllib.error
import urllib.request


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--env-file', required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    compose = ['sudo','docker','compose','--env-file',args.env_file,'-f',str(root/'deploy/compose.production.yaml')]
    def run(argv, **kwargs):
        result = subprocess.run(argv,capture_output=True,text=True,**kwargs)
        assert result.returncode == 0, 'Private smoke operation failed; sensitive diagnostics suppressed'
        return result.stdout.strip()
    config = json.loads(run(compose+['config','--format','json']))
    assert config['name'].endswith('-smoke'), 'Refusing non-smoke project'
    assert Path(config['secrets']['app_database_url']['file']).parent.parent.name == 'smoke'
    base = 'http://127.0.0.1:'+str(config['services']['frontend']['ports'][0]['published'])
    def sql(statement):
        return run(compose+['exec','-T','postgres','psql','-qAt','-v','ON_ERROR_STOP=1','-U','imt_owner','-d','imt_smart_teens','-c',statement])
    before = int(sql('SELECT count(*) FROM identity_admins'))
    email='seed-'+secrets.token_hex(8)+'@example.invalid'
    password=secrets.token_urlsafe(32)
    env={**os.environ,'IMT_ENV_FILE':args.env_file,'ADMIN_EMAIL':email,'ADMIN_NAME':'Synthetic container operator'}
    env.pop('ADMIN_PASSWORD',None)
    first=run(['sh',str(root/'deploy/operations.sh'),'seed-admin'],env=env,input=password)
    assert 'admin created' in first and password not in first
    query="SELECT id::text||':'||password_hash||':'||active::text FROM identity_admins WHERE email='"+email+"'"
    snapshot=sql(query)
    second=run(['sh',str(root/'deploy/operations.sh'),'seed-admin'],env=env,input=secrets.token_urlsafe(32))
    assert 'admin unchanged' in second and sql(query)==snapshot
    assert int(sql('SELECT count(*) FROM identity_admins'))==before+1
    # False legacy flags cannot reduce this active admin's global capabilities.
    sql("UPDATE identity_admins SET can_manage_schools=false,can_manage_content=false WHERE email='"+email+"'")
    cookies={}
    def request(path,method='GET',body=None,token=None):
        headers={'Origin':'https://imtsmartteens.my.id','Content-Type':'application/json','Cookie':'; '.join(k+'='+v for k,v in cookies.items())}
        if token:headers['X-CSRF-Token']=token
        req=urllib.request.Request(base+'/api/v1'+path,data=json.dumps(body).encode() if body is not None else None,headers=headers,method=method)
        try: response=urllib.request.urlopen(req,timeout=15)
        except urllib.error.HTTPError as response_error:response=response_error
        for raw in response.headers.get_all('Set-Cookie') or []:
            jar=SimpleCookie();jar.load(raw)
            for key,value in jar.items():cookies[key]=value.value
        raw=response.read()
        return response.status,json.loads(raw) if raw else None
    status,csrf=request('/csrf');assert status==200
    status,login=request('/admin/sessions','POST',{'email':email,'password':password},csrf['data']['token']);assert status==201
    token=login['data']['csrf_token']
    for path in ['/admin/me','/admin/schools','/admin/lessons','/admin/question-banks','/admin/students']:
        status,result=request(path);assert status==200
        if path=='/admin/me':
            assert result['data']['admin']['can_manage_schools'] and result['data']['admin']['can_manage_content']
    assert request('/admin/sessions/current','DELETE',token=token)[0]==204
    assert request('/admin/me')[0]==401
    print('PASS: actual container entrypoint, sudo-safe operator inputs, stdin credential, created/existing idempotence, Argon2-backed login, global legacy-false account access and logout; smoke-only')


if __name__ == '__main__': main()
