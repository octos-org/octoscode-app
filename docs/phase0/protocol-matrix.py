import re, os, sys, csv
W=os.environ.get('SRC_WEB','/Users/yuechen/home/oa.noindex/src-web')
N='/Users/yuechen/home/Octoscript-AppCard/app'
CORE='/Users/yuechen/home/Octoscript-AppCard/octos/crates/octos-core/src/ui_protocol.rs'
src=open(f'{W}/packages/client/src/generated/core-contract.ts').read()
def block(name):
    m=re.search(r'export const '+name+r'\b[^=]*=\s*[\[{](.*?)[\]}]\s*as const',src,re.S); return m.group(1) if m else ''
consts=dict(re.findall(r'([A-Z_]+):\s*"([^"]+)"',block('CORE_UI_METHODS')))
server=set(re.findall(r'"([^"]+)"',block('CORE_UI_SERVER_METHODS')))
notif=set(re.findall(r'"([^"]+)"',block('CORE_UI_NOTIFICATION_METHODS')))
def files(root,exts,skip=('node_modules','target','generated')):
    for d,ds,fs in os.walk(root):
        ds[:]=[x for x in ds if x not in skip and not x.startswith('.')]
        for f in fs:
            if f.endswith(exts): yield os.path.join(d,f)
web=[(p,open(p,errors='ignore').read()) for p in files(f'{W}/apps',('.ts','.tsx'))]+[(p,open(p,errors='ignore').read()) for p in files(f'{W}/packages',('.ts','.tsx'))]
nat=[(p,open(p,errors='ignore').read()) for p in files(N,('.rs',))]
core=open(CORE).read()
rows=[]
for c,m in sorted(consts.items(),key=lambda x:x[1]):
    pat_w=re.compile(r'\b'+c+r'\b|["\']'+re.escape(m)+r'["\']')
    wp=[p for p,t in web if pat_w.search(t)]
    wsrc=[p for p in wp if not re.search(r'\.(test|spec)\.',p)]
    wtest=[p for p in wp if re.search(r'\.(test|spec)\.',p)]
    ev=''.join(w.capitalize() for w in re.split(r'[/_.]',m))+'Event'
    np_=[p for p,t in nat if f'"{m}"' in t or re.search(r'methods::'+c+r'\b',t) or re.search(r'\b'+ev+r'\b',t)]
    kind='notification' if m in notif else ('request' if m in server else '?')
    rows.append(dict(method=m,kind=kind,web_src=len(wsrc),web_tests=len(wtest),native_files=len(np_),in_appcard_core=int(f'"{m}"' in core),
                     native_where=';'.join(sorted({os.path.relpath(p,N) for p in np_})[:3])))
with open(sys.argv[1],'w',newline='') as f:
    w=csv.DictWriter(f,fieldnames=list(rows[0])); w.writeheader(); w.writerows(rows)
used=[r for r in rows if r['web_src']>0]
print('contract methods',len(rows),'| requests',sum(r['kind']=='request' for r in rows),'| notifications',sum(r['kind']=='notification' for r in rows))
print('used by web src',len(used),'| of those native has',sum(r['native_files']>0 for r in used),'| missing natively',sum(r['native_files']==0 for r in used))
print('missing from AppCard vendored octos-core:',[r['method'] for r in rows if not r['in_appcard_core']])
print('\nWEB-USED, NATIVE-MISSING:'); print(' '.join(r['method']+('*' if r['kind']=='notification' else '') for r in used if r['native_files']==0))
print('\nWEB-USED, NATIVE-HAS:'); print(' '.join(r['method'] for r in used if r['native_files']>0))
