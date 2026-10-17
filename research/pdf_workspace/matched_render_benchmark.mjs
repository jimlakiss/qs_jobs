// Equal-resolution, full-page renderer comparison; no previews, tiles or Jobs UI.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
const root=fileURLToPath(new URL('./',import.meta.url));
const modules=process.env.IQS_TEST_NODE_MODULES;
if(!modules)throw new Error('Set IQS_TEST_NODE_MODULES');
const require=createRequire(import.meta.url);
const {chromium}=require(path.join(modules,'playwright'));
const source=process.argv[2];if(!source || !fs.existsSync(source))throw new Error('Supply local PDF path');
const legacy=path.join(root,'runtime/pdfjs-3.11.174');const modern=path.join(modules,'pdfjs-dist');
const server=http.createServer((req,res)=>{
 const url=new URL(req.url,'http://localhost');let data;let mime='application/javascript';
 if(url.pathname.startsWith('/h/')){
  const version=url.pathname.endsWith('3')?'3':'5';
  data='<!doctype html><input id="source" type="file"><script'+(version==='5'?' type="module"':'')+'>'+
   (version==='5'?"window.pdfjsLib=await import('/5/build/pdf.mjs');":'')+'</script>'+
   (version==='3'?'<script src="/3/build/pdf.min.js"></script>':'');mime='text/html';
 }else{
  const base=url.pathname.startsWith('/3/')?legacy:url.pathname.startsWith('/5/')?modern:null;
  if(!base){res.writeHead(404);res.end();return;}
  const target=path.resolve(base,url.pathname.slice(3));
  if(!target.startsWith(base+path.sep)||!fs.existsSync(target)){res.writeHead(404);res.end();return;}
  data=fs.readFileSync(target);if(!/\.m?js$/.test(target))mime='application/octet-stream';
 }
 res.writeHead(200,{'Content-Type':mime,'Cache-Control':'no-store'});res.end(data);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const address=`http://127.0.0.1:${server.address().port}`;
const report={kind:'matched_full_page_comparison',source_bytes:fs.statSync(source).size,dpi:136,
 trials_per_page:5,warm_renders_per_trial:3,engine_order:['PDF.js 3.11.174','PDFium 7881 / Rust','PDF.js 5.6.205'],
 method:'same PDF, Mac and full-page pixel dimensions; no previews/tiles/UI; source bytes and engine/worker initialization before timers; fresh document per trial; five first renders and fifteen warm renders per page; readback reported separately; OS/font caches uncontrolled',engines:[]};
let browser;
const bounded=(promise,ms,label)=>{let timer;return Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error(label+' timed out')),ms);})]).finally(()=>clearTimeout(timer));};
try{
 browser=await chromium.launch({headless:true,executablePath:process.env.IQS_CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
 report.browser=browser.version();
 for(const version of ['3','rust','5']){
  if(version==='rust'){
   const result=JSON.parse(execFileSync(path.join(root,'target/release/iqs-pdf-spike'),['--matched-benchmark',source],{encoding:'utf8',maxBuffer:4*1024*1024,timeout:120000}));
   for(const p of result.pages)p.status='success';report.engines.push(result);console.log('Rust completed');continue;
  }
  const engine={engine:version==='3'?'PDF.js 3.11.174':`PDF.js ${require(path.join(modern,'package.json')).version}`,pages:[]};
  report.engines.push(engine);let count=1;
  for(let number=1;number<=count;number++){
   const context=await browser.newContext();
   await context.route('**/*',route=>route.request().url().startsWith(address+'/')?route.continue():route.abort());
   const tab=await context.newPage();let rejectError;const errorSignal=new Promise((_,reject)=>{rejectError=reject;});
   const errors=[];tab.on('pageerror',e=>{errors.push(e.stack||String(e));rejectError(e);});
   let result;
   try{
    await tab.goto(address+'/h/'+version,{waitUntil:'networkidle'});await tab.waitForFunction(()=>!!window.pdfjsLib);
    await tab.locator('#source').setInputFiles(source);
    result=await bounded(Promise.race([tab.evaluate(async({number,version})=>{
     const lib=window.pdfjsLib;const prefix='/'+version+'/';
     lib.GlobalWorkerOptions.workerSrc=prefix+'build/pdf.worker.'+(version==='3'?'min.js':'mjs');
     const worker=new lib.PDFWorker({name:'equal-resolution-benchmark'});await worker.promise;
     const bytes=new Uint8Array(await document.getElementById('source').files[0].arrayBuffer());
     const trials=[];let width=0,height=0,pages=0;
     for(let trial=0;trial<5;trial++){
      const data=bytes.slice();const opened=performance.now();
      const doc=await lib.getDocument({data,worker,cMapUrl:prefix+'cmaps/',cMapPacked:true,
       standardFontDataUrl:prefix+'standard_fonts/',wasmUrl:prefix+'wasm/',iccUrl:prefix+'iccs/',
       disableAutoFetch:true,disableStream:false,disableFontFace:false,useSystemFonts:false}).promise;
      const open_ms=performance.now()-opened;pages=doc.numPages;
      const first=performance.now();const pg=await doc.getPage(number);
      const viewport=pg.getViewport({scale:136/72});width=Math.floor(viewport.width);height=Math.floor(viewport.height);
      const canvas=document.createElement('canvas');canvas.width=width;canvas.height=height;const ctx=canvas.getContext('2d');
      await pg.render({canvasContext:ctx,viewport,intent:'display',renderInteractiveForms:false,enableWebGL:false}).promise;
      const first_render_ms=performance.now()-first;
      let copied=performance.now();let image=ctx.getImageData(0,0,width,height);const first_readback_ms=performance.now()-copied;
      if(image.data.length!==width*height*4)throw new Error('Unexpected bitmap byte count');image=null;canvas.width=0;canvas.height=0;
      const warm=[];
      for(let i=0;i<3;i++){
       const start=performance.now();const c=document.createElement('canvas');c.width=width;c.height=height;const cx=c.getContext('2d');
       await pg.render({canvasContext:cx,viewport,intent:'display',renderInteractiveForms:false,enableWebGL:false}).promise;
       const render_ms=performance.now()-start;copied=performance.now();let px=cx.getImageData(0,0,width,height);
       const readback_ms=performance.now()-copied;if(px.data.length!==width*height*4)throw new Error('Incomplete raster');px=null;
       warm.push({render_ms,readback_ms});c.width=0;c.height=0;
      }
      trials.push({open_ms,first_render_ms,first_total_ms:open_ms+first_render_ms,first_readback_ms,warm});
      pg.cleanup();await doc.destroy();
     }
     await worker.destroy();return {page:number,pages,width,height,status:'success',trials};
    },{number,version}),errorSignal]),30000,engine.engine+' page '+number);
    count=result.pages;
   }catch(error){result={page:number,status:'failure',error:String(error),errors};}
   engine.pages.push(result);console.log(engine.engine+' page '+number+': '+result.status);
   await context.close();
  }
 }
}finally{
 if(browser)await browser.close();server.closeAllConnections();await new Promise(resolve=>server.close(resolve));
 fs.writeFileSync(path.join(root,'results/structural-matched-render.json'),JSON.stringify(report,null,2)+'\n');
 console.log('Saved results/structural-matched-render.json');
}
