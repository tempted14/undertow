"""Repeatable generated playback checks. Windows only; creates no visible windows.
Usage: python scripts/performance-compare.py PHASE EXE [--only-gpu]
Outputs use ignored artifacts/performance storage with an isolated account-free data directory.
"""
from pathlib import Path
import runpy,sys,os,json,subprocess,hashlib
root=Path(__file__).resolve().parents[1];work=root/'artifacts/performance';work.mkdir(parents=True,exist_ok=True);phase=sys.argv[1];exe=Path(sys.argv[2]).resolve();out=work/phase;out.mkdir(exist_ok=True)
fixture=work/'fixture';fixture.mkdir(exist_ok=True);runpy.run_path(str(root/'scripts/ui-smoke.py'))['fixtures'](fixture)
(fixture/'spaced.wav').write_bytes((fixture/'motion.wav').read_bytes());(fixture/'spaced.png').write_bytes((fixture/'motion.png').read_bytes());(fixture/'spaced.lrc').write_text('\n'.join(f'[00:{i:02d}.00]Original steady lyric line {i}, flowing between changes' for i in range(0,45,6)),encoding='utf-8')
(fixture/'words.wav').write_bytes((fixture/'motion.wav').read_bytes());(fixture/'words.png').write_bytes((fixture/'motion.png').read_bytes());(fixture/'words.lrc').write_text('\n'.join(f'[00:{i:02d}.00]<00:{i:02d}.00>Original <00:{i+1:02d}.00>word <00:{i+2:02d}.00>timing <00:{i+3:02d}.00>fixture' for i in range(0,40,6)),encoding='utf-8')
if os.name != 'nt': raise SystemExit('Native Windows diagnostics required')
if not phase.replace('-', '').replace('_', '').isalnum(): raise SystemExit('Use a simple phase name')
results=[]
for name,audio,extra in [('tracks','spaced',['--preview-library']),('line-lyrics','spaced',['--lyrics']),('word-lyrics','words',['--lyrics']),('gpu-line-lyrics','spaced',['--lyrics','--gpu-benchmark'])]:
 if len(sys.argv)>3 and not name.startswith('gpu-'):continue
 data=work/(phase+'-'+name+'-data');data.mkdir(exist_ok=True);(data/'settings.json').write_text(json.dumps({'discord':False,'floating_lyrics':False,'everywhere':False,'show_sidebar':False,'fps':120}),encoding='utf-8');env=os.environ.copy();env['UNDERTOW_DATA_DIR']=str(data)
 cmd=[str(exe),'--background-benchmark','--benchmark','18','--fps','120','--mute','--open',str(fixture/(audio+'.wav')),'--window-size','1600x1000','--report',str(out/(name+'.json')),'--screenshot',str(out/(name+'.png')),*extra]
 p=subprocess.Popen(cmd,env=env,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
 gpu=None
 if name.startswith('gpu-'):
  gpu=subprocess.Popen(['pwsh','-NoProfile','-File',str(root/'scripts/gpu-sample.ps1'),'-TaskProcessId',str(p.pid),'-OutputPath',str(out/'gpu-counters.json')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS)
 p.wait(timeout=70)
 if gpu:gpu.wait(timeout=40)
 if p.returncode or not (out/(name+'.json')).exists():results.append({'name':name,'failed':True,'exit_code':p.returncode});print(name,'unavailable',flush=True);continue
 r=json.loads((out/(name+'.json')).read_text(encoding='utf-8'));d=r['details'];results.append({'name':name,'renderer':d['renderer'],'working_set_bytes':r['peak_working_set_bytes'],'private_bytes':r['peak_private_bytes'],'cpu_percent_one_core':r['cpu_percent_one_core'],'ui_frames':d.get('ui_frames'),'underrun_samples':d.get('underrun_samples'),'consumed_audio_seconds':d.get('consumed_audio_seconds'),'duration_seconds':r['duration_seconds']});print(phase,name,flush=True)
(out/'summary.json').write_text(json.dumps({'sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'results':results,'scope':'18-second hidden 1600x1000, requested 120fps with display sync enabled; original local audio/art/lyric fixtures, muted output and isolated data; includes hidden OpenGL if available. Actual UI-frame counts are reported separately from the requested setting. Whole-process CPU, peak working set and private bytes. GPU utilization not inferred from CPU or frame counts.'},indent=2),encoding='utf-8')
