"""Measure the same view sequence in fresh native and dedicated Chrome process trees.
RSS sums can double-count shared pages and exclude nonresident/driver allocations.
The Node controller is deliberately outside the browser accounting tree.
"""
import argparse, datetime, hashlib, json, os, selectors, signal, statistics, subprocess, time
from pathlib import Path
ROOT=Path(__file__).resolve().parent

def process_tree(root):
    rows={}
    result=subprocess.run(['ps','-axo','pid=,ppid=,rss='],capture_output=True,text=True)
    for line in result.stdout.splitlines():
        parts=line.split()
        if len(parts)==3 and all(p.isdigit() for p in parts):
            pid,ppid,kib=map(int,parts);rows[pid]=(ppid,kib*1024)
    selected={root}
    while True:
        added={pid for pid,(ppid,_) in rows.items() if ppid in selected}-selected
        if not added:break
        selected.update(added)
    return {pid:rows[pid][1] for pid in selected if pid in rows}

def measure(engine,source,workflow,trial):
    env=dict(os.environ,IQS_MEMORY_WORKFLOW=str(workflow))
    cmd=[str(ROOT/'target/release/iqs-pdf-spike'),str(source)] if engine=='rust' else [env.get('IQS_NODE','node'),str(ROOT/'memory_browser_workflow.mjs'),str(source)]+(['--modern'] if engine=='pdfjs5' else [])
    name=f'{source.stem[:20]}-{engine}-{trial}'
    stderr=(ROOT/'results'/f'memory-{name}.log').open('w')
    proc=subprocess.Popen(cmd,cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=stderr,start_new_session=True,bufsize=0)
    sel=selectors.DefaultSelector();sel.register(proc.stdout,selectors.EVENT_READ)
    root=proc.pid if engine=='rust' else None
    stage='startup';samples=[];events=[];started=time.monotonic();settled=None; pending=b''; last_footprint=0
    footprint_path=ROOT/'results'/f'footprint-{name}.json'
    while proc.poll() is None:
        for key,_ in sel.select(timeout=0.1):
            pending+=os.read(key.fileobj.fileno(),65536)
            while b'\n' in pending:
                line,pending=pending.split(b'\n',1)
                try:event=json.loads(line)
                except (json.JSONDecodeError,ValueError):continue
                event['time']=time.monotonic()-started;events.append(event)
                if event.get('event')=='root_pid':root=event['pid']
                if event.get('event')=='phase':stage=event['name'];settled=None
                if event.get('event')=='ready':settled=time.monotonic()
                if event.get('event') in ('settled','complete','failed'):print(json.dumps({'run':name,**event}),flush=True)
        if root:
            values=process_tree(root)
            if values and sum(values.values())>0:
                physical=None
                if time.monotonic()-last_footprint>=0.5:
                    last_footprint=time.monotonic()
                    try:
                        result=subprocess.run(['/usr/bin/footprint','--noCategories','-f','bytes','-j',str(footprint_path),*map(str,sorted(values))],capture_output=True,timeout=5)
                        if result.returncode==0:
                            physical=json.loads(footprint_path.read_text())['total footprint']
                    except (OSError,subprocess.TimeoutExpired,KeyError,json.JSONDecodeError):pass
                samples.append({'t':time.monotonic()-started,'phase':stage,'ready':settled is not None,'rss':sum(values.values()),'footprint':physical,'processes':len(values)})
        if time.monotonic()-started>300:
            events.append({'event':'failed','message':'300s runner deadline'})
            os.killpg(proc.pid,signal.SIGKILL);break
    proc.wait();stderr.close();sel.close()
    # Tail completion marker may have been buffered alongside an earlier line.
    for line in (pending+proc.stdout.read()).splitlines():
        try:events.append(json.loads(line))
        except json.JSONDecodeError:pass
    phases={}
    for phase_name in dict.fromkeys(s['phase'] for s in samples):
        values=[s['rss'] for s in samples if s['phase']==phase_name]
        stable=[s['rss'] for s in samples if s['phase']==phase_name and s['ready']]
        physical=[s['footprint'] for s in samples if s['phase']==phase_name and s.get('footprint') is not None]
        stable_physical=[s['footprint'] for s in samples if s['phase']==phase_name and s['ready'] and s.get('footprint') is not None]
        phases[phase_name]={'footprint_peak_bytes':max(physical) if physical else None,'footprint_settled_bytes':statistics.median(stable_physical) if stable_physical else None,'peak_bytes':max(values),'settled_median_bytes':statistics.median(stable) if stable else None,'last_bytes':values[-1],'samples':len(values)}
    baseline=phases.get('baseline',{}).get('settled_median_bytes')
    report={'engine':engine,'source':source.name,'trial':trial,'success':any(e.get('event')=='complete' and e.get('success') for e in events),'events':events,'phases':phases,'peak_bytes':max((s['rss'] for s in samples),default=0),'baseline_bytes':baseline,'samples':samples,'returncode':proc.returncode}
    report['footprint_peak_bytes']=max((s['footprint'] for s in samples if s.get('footprint') is not None),default=0)
    report['footprint_baseline_bytes']=phases.get('baseline',{}).get('footprint_settled_bytes')
    report['incremental_peak_bytes']=report['peak_bytes']-baseline if baseline else None
    (ROOT/'results'/f'memory-{name}.json').write_text(json.dumps(report,indent=2)+'\n')
    return report

def main():
    (ROOT/'results').mkdir(exist_ok=True)
    ap=argparse.ArgumentParser();ap.add_argument('source',type=Path);ap.add_argument('--engines',nargs='+',default=['rust','pdfjs3','pdfjs5']);ap.add_argument('--trials',type=int,default=1);ap.add_argument('--pages',type=int,required=True);args=ap.parse_args()
    steps=[{'name':'baseline','page':0,'zoom':1,'pan':[0,0],'dwell':3}]
    def step(name,page,zoom=1,pan=(0,0),dwell=2):steps.append(dict(name=name,page=page,zoom=zoom,pan=pan,dwell=dwell))
    step('open_fit_width',1,dwell=3)
    for p in [2,3,4,args.pages,1]:step(f'first_sheet_{p}',p)
    step('zoom_2x',1,2);step('pan_2x',1,2,(-.2,-.15));step('zoom_4x',1,4);step('pan_4x',1,4,(.2,.15))
    for cycle in range(2):
        for p in [2,args.pages,1]:step(f'return_{cycle}_sheet_{p}',p)
    step('idle_after_navigation',1,dwell=8)
    workflow=ROOT/'results'/f'memory-workflow-{args.pages}.json';workflow.write_text(json.dumps(steps))
    reports=[]
    for trial in range(1,args.trials+1):
        # Reverse engine order on the second run to reduce order bias.
        for engine in (args.engines if trial%2 else list(reversed(args.engines))):
            print(json.dumps({'starting':engine,'trial':trial,'source':args.source.name}),flush=True)
            reports.append(measure(engine,args.source.resolve(),workflow,trial))
    output=ROOT/'results'/f'memory-comparison-footprint-{args.pages}.json'
    output.write_text(json.dumps({'date':datetime.datetime.now(datetime.timezone.utc).isoformat(),'method':'macOS footprint total across the selected process tree sampled about every 0.5 seconds; simultaneous totals, not sum of historical per-process peaks; plus summed process-tree RSS sampled roughly every 100-200 ms; shared mappings may be counted more than once; excludes nonresident and unreported GPU driver allocations; browser controller excluded; fresh dedicated headed Chrome; no forced GC; no overlays/snapping','source_bytes':args.source.stat().st_size,'source_sha256':hashlib.file_digest(args.source.open('rb'),'sha256').hexdigest(),'viewport_css_points':[900,650],'device_scale':2,'workflow':steps,'runs':reports},indent=2)+'\n')
    print('Saved '+str(output),flush=True)
if __name__=='__main__':main()
