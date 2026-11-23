"""Repeatable generated playback checks. Windows only; creates no visible windows.
Usage: python scripts/performance-compare.py PHASE EXE [--only-gpu]
Outputs use ignored artifacts/performance storage with an isolated account-free data directory.
"""
from pathlib import Path
import argparse,runpy,sys,os,json,subprocess,hashlib,math,struct,wave
cases=[('tracks','spaced',['--preview-library']),('line-lyrics','spaced',['--lyrics']),('gpu-line-lyrics','spaced',['--lyrics','--gpu-benchmark']),('high-rate-tracks','high-rate',['--preview-library'])]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('phase');parser.add_argument('executable');parser.add_argument('--only-gpu',action='store_true')
parser.add_argument('--cases',nargs='+',choices=[c[0] for c in cases],help='Run named cases and retain other results from this phase for the same executable.')
args=parser.parse_args();phase=args.phase;exe=Path(args.executable).resolve()
if os.name != 'nt':parser.error('Native Windows diagnostics required')
if not phase.replace('-', '').replace('_', '').isalnum():parser.error('Use a simple phase name')
if not exe.is_file():parser.error('Executable does not exist')
root=Path(__file__).resolve().parents[1];work=root/'artifacts/performance';work.mkdir(parents=True,exist_ok=True);out=work/phase;out.mkdir(exist_ok=True)
fixture=work/'fixture';fixture.mkdir(exist_ok=True);runpy.run_path(str(root/'scripts/ui-smoke.py'))['fixtures'](fixture)
(fixture/'spaced.wav').write_bytes((fixture/'motion.wav').read_bytes());(fixture/'spaced.png').write_bytes((fixture/'motion.png').read_bytes());(fixture/'spaced.lrc').write_text('\n'.join(f'[00:{i:02d}.00]Original steady lyric line {i}, flowing between changes' for i in range(0,45,6)),encoding='utf-8')
selected=[c for c in cases if (c[0] in args.cases if args.cases else c[1]!='high-rate') and (not args.only_gpu or c[0].startswith('gpu-'))]
if not selected:parser.error('No matching cases selected')
if any(c[1]=='high-rate' for c in selected) and not (fixture/'high-rate.wav').exists():
 second=b''.join(struct.pack('<ii',int(40000000*math.sin(2*math.pi*220*i/192000)),int(30000000*math.sin(2*math.pi*330*i/192000))) for i in range(192000))
 with wave.open(str(fixture/'high-rate.wav'),'wb') as audio:
  audio.setparams((2,4,192000,0,'NONE','not compressed'))
  for _ in range(30):audio.writeframesraw(second)
 (fixture/'high-rate.png').write_bytes((fixture/'motion.png').read_bytes())
digest=hashlib.sha256(exe.read_bytes()).hexdigest();results=[]
if args.cases and (out/'summary.json').exists():
 previous=json.loads((out/'summary.json').read_text(encoding='utf-8'))
 if previous['sha256']!=digest:parser.error('Cannot combine different executables in one phase')
 results=[r for r in previous['results'] if r['name'] not in [c[0] for c in selected]]
for name,audio,extra in selected:
 data=work/(phase+'-'+name+'-data');data.mkdir(exist_ok=True);(data/'settings.json').write_text(json.dumps({'discord':False,'floating_lyrics':False,'everywhere':False,'show_sidebar':False,'fps':120}),encoding='utf-8');env=os.environ.copy();env['UNDERTOW_DATA_DIR']=str(data)
 cmd=[str(exe),'--background-benchmark','--benchmark','18','--fps','120','--mute','--open',str(fixture/(audio+'.wav')),'--window-size','1600x1000','--report',str(out/(name+'.json')),'--screenshot',str(out/(name+'.png')),*extra]
 with (out/(name+'.log')).open('wb') as log:
  p=subprocess.Popen(cmd,env=env,stdout=log,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
 gpu=None
 if name.startswith('gpu-'):
  gpu=subprocess.Popen(['pwsh','-NoProfile','-File',str(root/'scripts/gpu-sample.ps1'),'-TaskProcessId',str(p.pid),'-OutputPath',str(out/('gpu-counters.json' if name=='gpu-line-lyrics' else name+'-gpu-counters.json'))],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
 p.wait(timeout=70)
 if gpu:gpu.wait(timeout=40)
 if p.returncode or not (out/(name+'.json')).exists():results.append({'name':name,'failed':True,'exit_code':p.returncode});print(name,'unavailable',flush=True);continue
 r=json.loads((out/(name+'.json')).read_text(encoding='utf-8'));d=r['details'];results.append({'name':name,'renderer':d['renderer'],'working_set_bytes':r['peak_working_set_bytes'],'private_bytes':r['peak_private_bytes'],'cpu_percent_one_core':r['cpu_percent_one_core'],'ui_frames':d.get('ui_frames'),'underrun_samples':d.get('underrun_samples'),'consumed_audio_seconds':d.get('consumed_audio_seconds'),'duration_seconds':r['duration_seconds']});print(phase,name,flush=True)
(out/'summary.json').write_text(json.dumps({'sha256':digest,'results':results,'scope':'18-second hidden 1600x1000, requested 120fps with display sync enabled; original local audio/art/lyric fixtures, muted output and isolated data; includes hidden OpenGL if available. Actual UI-frame counts are reported separately from the requested setting. Whole-process CPU, peak working set and private bytes. GPU utilization not inferred from CPU or frame counts.'},indent=2),encoding='utf-8')
if any(r.get('failed') for r in results):raise SystemExit('A workload failed; inspect the ignored per-case log before publishing.')
