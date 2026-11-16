# Regenerates licenses/python-packages.tsv from the uv.lock files: `python3 -I scripts/python-licences.py . licenses/python-packages.tsv`. Reads PyPI.
import json, sys, tomllib, urllib.request, time
root=sys.argv[1]; out=sys.argv[2]
groups={'worker':'python/uv.lock','kokoro':'plugins/kokoro/uv.lock','whisper':'plugins/whisper/uv.lock'}
cache={}
rows=[]
def lic(n,v):
    if (n,v) in cache: return cache[(n,v)]
    r='not published'
    try:
        d=json.load(urllib.request.urlopen(f'https://pypi.org/pypi/{n}/{v}/json',timeout=15))['info']
        e=d.get('license_expression')
        l=(d.get('license') or '').strip()
        cl=[c.split('::')[-1].strip() for c in d.get('classifiers',[]) if c.startswith('License ::')]
        if e: r=e
        elif cl: r=' / '.join(cl)
        elif l and len(l)<60: r=l.replace('\t',' ').replace('\n',' ')
    except Exception as ex:
        r='not read: '+type(ex).__name__
    cache[(n,v)]=r; time.sleep(.05); return r
for g,f in groups.items():
    d=tomllib.load(open(f'{root}/{f}','rb'))
    for p in d['package']:
        s=p.get('source',{})
        if s.get('virtual') or s.get('editable'): continue
        rows.append((g,p['name'],p['version'],lic(p['name'],p['version'])))
with open(out,'w') as o:
    o.write('# Python packages of the worker and plugins, from their uv.lock files. Licence as PyPI states it. Regenerate with scripts/python-licences.py.\n')
    for r in sorted(rows): o.write('\t'.join(r)+'\n')
print(len(rows))
