"""Read-only text/bitmap alignment checks against the actual native worker."""
import argparse,json,math,os,subprocess
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).resolve().parent

def inspect(source,page=0):
    process=subprocess.Popen([str(ROOT/'target/release/iqs-pdf-spike'),'--pdf-worker',str(source)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    seq=0
    def request(level,doc=''):
        nonlocal seq
        seq+=1
        process.stdin.write((json.dumps({'seq':seq,'key':{'doc':doc,'page':page,'level':level,'col':0,'row':0}})+'\n').encode());process.stdin.flush()
        header=json.loads(process.stdout.readline(16385));data=process.stdout.read(header['bytes'])
        assert not header['error'],header['error']
        return header,data
    try:
        header,_=request(-2);info=header['info'];doc=info['doc']
        header,pixels=request(-1,doc);image=Image.frombytes('RGBA',tuple(header['size']),pixels)
        header,data=request(-3,doc);text=json.loads(data)
        assert len(data)<=8*1024*1024 and len(text['glyphs'])<=50000
        positioned=[g for g in text['glyphs'] if g[1] and all(0<=v<=1 for v in g[1])]
        hits=0
        for char,b in positioned:
            box=(max(0,math.floor(b[0]*image.width)-1),max(0,math.floor(b[1]*image.height)-1),min(image.width,math.ceil(b[2]*image.width)+1),min(image.height,math.ceil(b[3]*image.height)+1))
            if any(min(p[:3])<200 for p in image.crop(box).get_flattened_data()):hits+=1
        return {'file':source.name,'page':page+1,'rotation':info['rotation'],'characters':len(text['glyphs']),'positioned':len(positioned),'bitmap_overlap':hits,'overlap_fraction':hits/max(1,len(positioned)),'payload_bytes':len(data),'truncated':text['truncated'],'plain_text':''.join(g[0] for g in text['glyphs'])}
    finally:
        process.stdin.close();process.wait(timeout=5)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--source',type=Path);args=parser.parse_args()
    results=[]
    for name in ['vector.pdf','rotated_crop.pdf','raster.pdf']:
        result=inspect(ROOT/'fixtures'/name)
        if name=='raster.pdf':assert result['characters']==0
        else:
            assert 'iQs synthetic drawing - A101' in result['plain_text']
            assert result['overlap_fraction']>=0.95,result
        del result['plain_text'];results.append(result)
    if args.source:
        for page in [0,1,7]:
            result=inspect(args.source,page);del result['plain_text'];results.append(result)
    print(json.dumps(results,indent=2))
    (ROOT/'results/text-layer-checks.json').write_text(json.dumps(results,indent=2)+'\n')
if __name__=='__main__':main()
