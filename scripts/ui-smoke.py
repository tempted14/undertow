"""Create original fixtures and render invisible UI smoke checks on Windows."""
import argparse,json,math,os,struct,subprocess,tempfile,wave,zlib
from pathlib import Path

SCREENS=['home','tracks','playlist','album','collection-menu','collection-playlists','collection-albums','artist','search','search-preview','lyrics','credits','similar','general','account','plugins','about','profile','queue','saved','create','edit','delete','volume','error']

def fixtures(folder):
    audio=folder/'motion.wav'
    with wave.open(str(audio),'wb') as f:
        f.setparams((1,2,44100,0,'NONE','not compressed'))
        second=b''.join(struct.pack('<h',int(1000*math.sin(2*math.pi*220*i/44100))) for i in range(44100))
        for _ in range(45):f.writeframesraw(second)
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
    side=256
    pixels=b''.join(b'\0'+b''.join(bytes((32+x//2,20+y//3,80+(x+y)//5)) for x in range(side)) for y in range(side))
    (folder/'motion.png').write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',side,side,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(pixels))+chunk(b'IEND',b''))
    (folder/'motion.lrc').write_text('\n'.join(f'[00:{i:02d}.00]Original interface check line {i:02d}, keep the motion flowing' for i in range(0,45,2)),encoding='utf-8')
    return audio
def main():
    p=argparse.ArgumentParser()
    p.add_argument('--executable',default='target/release/undertow.exe')
    p.add_argument('--output',default='artifacts/ui')
    p.add_argument('--screens',nargs='+',choices=SCREENS,default=SCREENS)
    p.add_argument('--size',default='1200x900')
    p.add_argument('--sidebar',action='store_true',help='Show the library sidebar')
    p.add_argument('--scale',type=float,choices=[1.,1.25,1.5,1.75,2.],default=1.,help='Simulate effective UI scaling in the hidden app')
    a=p.parse_args()
    if os.name!='nt':p.error('These native checks require Windows.')
    exe=Path(a.executable).resolve();out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='undertow-smoke-') as temp:
        folder=Path(temp);audio=fixtures(folder);data=folder/'data';data.mkdir()
        (data/'settings.json').write_text(json.dumps({'discord':False,'floating_lyrics':False,'everywhere':False,'profile_picture':False,'show_sidebar':a.sidebar,'fps':30,'download_dir':'Downloads'}),encoding='utf-8')
        env=os.environ.copy();env['UNDERTOW_DATA_DIR']=str(data)
        for screen in a.screens:
            subprocess.run([str(exe),'--background-benchmark','--benchmark','6','--mute','--fps','30','--open',str(audio),'--preview-screen',screen,'--window-size',a.size,'--preview-scale',str(a.scale),'--report',str(out/(screen+'.json')),'--screenshot',str(out/(screen+'.png'))],env=env,creationflags=subprocess.CREATE_NO_WINDOW|subprocess.BELOW_NORMAL_PRIORITY_CLASS,check=True,timeout=60)
            print(screen+' rendered',flush=True)
if __name__=='__main__':main()
