// Runs the actual Jobs HTML/JS locally, without Rails sessions or write URLs.
// Only ERB host config and dependency asset URLs are substituted.
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const root=fileURLToPath(new URL('./',import.meta.url));
const repo=path.resolve(root,'../..');
const modules=process.env.IQS_TEST_NODE_MODULES;
if(!modules)throw new Error('Set IQS_TEST_NODE_MODULES to the bundled Node modules directory');
const require=createRequire(import.meta.url);
const {chromium}=require(path.join(modules,'playwright'));
const source=process.argv[2];
const modern=process.argv.includes('--modern');
const skipThumbs=process.argv.includes('--skip-thumbnails');
const rawPage=process.argv.includes('--raw-page2');
const label=rawPage?'raw-page2':modern?'modern':skipThumbs?'legacy-no-thumbnails':'legacy';
if(!source || !fs.existsSync(source))throw new Error('Supply an existing local PDF path');
const assets=path.join(root,'runtime/pdfjs-3.11.174');
if(!fs.existsSync(path.join(assets,'build/pdf.min.js')))throw new Error('Extract PDF.js 3.11.174 into runtime/pdfjs-3.11.174 first');
let html=fs.readFileSync(path.join(repo,'app/views/project_documents/viewer.html.erb'),'utf8')
  .replace(/<script>\s*window\.qsJobsDocument\s*=[\s\S]*?<\/script>/,
    '<script>window.qsJobsDocument={documentCategory:"imported",navigationDocuments:[]};</script>')
  .replace(/<%[\s\S]*?%>/g,'')
  .replace('https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/pdf.min.js','/assets/build/pdf.min.js')
  .replace(/<script src="https:\/\/(?:unpkg\.com\/tesseract[^\"]*|cdnjs\.cloudflare\.com\/ajax\/libs\/(?:pdf-lib|jszip)[^\"]*)"><\/script>/g,'')
  .replace('</body>','<script src="/instrument.js"></script></body>');
if(modern)html=html
  .replace('<script src="/assets/build/pdf.min.js"></script>','<script type="module" src="/bootstrap.mjs"></script>')
  .replace(/<script src="\/pdf_viewer\/(?:pdf_viewer|pdf_splitter)\.js[^\"]*"><\/script>/g,'')
  .replace('<script src="/instrument.js"></script>','');
if(rawPage)html='<!doctype html><input type="file" id="file-input"><canvas id="pdf-canvas"></canvas><script src="/assets/build/pdf.min.js"></script><script>window.__diag={spans:[],gaps:[],longTasks:[],errors:[]};pdfjsLib.GlobalWorkerOptions.workerSrc="/assets/build/pdf.worker.min.js";</script>';
const instrument=`
window.__diag={spans:[],gaps:[],longTasks:[],errors:[]};
${skipThumbs?'window.loadThumbnail=async()=>{};':''}
let lastTick=performance.now();
setInterval(()=>{const now=performance.now();__diag.gaps.push(now-lastTick);lastTick=now;},16);
new PerformanceObserver(list=>{for(const e of list.getEntries())__diag.longTasks.push({start:e.startTime,duration:e.duration});}).observe({type:'longtask',buffered:true});
for(const name of ['handleSelectedPDFs','buildThumbnails','loadThumbnail','renderPage','msrBuildSnapCache']){
 const original=window[name];
 window[name]=async function(...args){const t=performance.now();const span={name,page:typeof args[0]==='number'?args[0]:null,start:t};__diag.spans.push(span);
 try{return await original.apply(this,args);}catch(e){span.error=String(e);throw e;}finally{span.end=performance.now();span.ms=span.end-t;}}
}
`;
const server=http.createServer((req,res)=>{
  const url=new URL(req.url,'http://localhost');
  let body;let mime='application/javascript';
  if(url.pathname==='/'){body=html;mime='text/html';}
  else if(url.pathname==='/instrument.js')body=instrument;
  else if(url.pathname==='/bootstrap.mjs')body=`import * as pdfjs from '/assets5/build/pdf.mjs';window.pdfjsLib=pdfjs;
for(const src of ['/pdf_viewer/pdf_viewer.js','/pdf_viewer/pdf_splitter.js','/instrument.js']){
 await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src=src;script.onload=resolve;script.onerror=reject;document.body.appendChild(script);});}`;
  else if(url.pathname==='/pdf_viewer/pdf_viewer.js'){
    body=fs.readFileSync(path.join(repo,'public/pdf_viewer/pdf_viewer.js'),'utf8')
      .replaceAll('https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/pdf.worker.min.js',modern?'/assets5/build/pdf.worker.mjs':'/assets/build/pdf.worker.min.js')
      .replaceAll('https://cdn.jsdelivr.net/npm/pdfjs-dist@3.11.174/cmaps/',modern?'/assets5/cmaps/':'/assets/cmaps/')
      .replaceAll('https://cdn.jsdelivr.net/npm/pdfjs-dist@3.11.174/standard_fonts/',modern?'/assets5/standard_fonts/':'/assets/standard_fonts/');
  }else if(url.pathname==='/pdf_viewer/pdf_splitter.js')body=fs.readFileSync(path.join(repo,'public/pdf_viewer/pdf_splitter.js'));
  else if(url.pathname.startsWith('/assets/') || url.pathname.startsWith('/assets5/')){
    const base=url.pathname.startsWith('/assets5/')?path.join(modules,'pdfjs-dist'):assets;
    const target=path.resolve(base,url.pathname.replace(/^\/assets5?\//,''));
    if(!target.startsWith(base+path.sep) || !fs.existsSync(target)){res.writeHead(404);res.end();return;}
    body=fs.readFileSync(target);if(!/\.m?js$/.test(target))mime='application/octet-stream';
  }else{res.writeHead(404);res.end();return;}
  res.writeHead(200,{'Content-Type':mime,'Cache-Control':'no-store'});res.end(body);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const address=`http://127.0.0.1:${server.address().port}`;

const workflow=JSON.parse(fs.readFileSync(process.env.IQS_MEMORY_WORKFLOW,'utf8'));
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const within=(promise,ms)=>{let timer;return Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('render deadline')),ms);})]).finally(()=>clearTimeout(timer));};
function mark(event, extra={}) { console.log(JSON.stringify({event,...extra})); }
let browserServer, browser;
try {
 browserServer=await chromium.launchServer({headless:false,executablePath:process.env.IQS_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
 mark('root_pid',{pid:browserServer.process().pid});
 browser=await chromium.connect(browserServer.wsEndpoint());
 mark('browser_version',{version:browser.version(),engine:modern?'5.6.205':'3.11.174'});
 const context=await browser.newContext({viewport:{width:1200,height:850},deviceScaleFactor:2});
 await context.route('**/*',route=>route.request().url().startsWith(address+'/')?route.continue():route.abort());
 const page=await context.newPage();
 const errors=[];
 page.on('pageerror',e=>errors.push(String(e)));
 page.on('dialog',d=>d.accept());
 await page.goto(address,{waitUntil:'networkidle'});
 await page.waitForFunction(()=>window.__diag);
 // Same drawing viewport as native; actual Jobs controls, thumbnail cache and renderer remain loaded.
 await page.addStyleTag({content:'#pdf-scroll { flex: none !important; width:900px !important; height:650px !important; min-width:900px !important; min-height:650px !important; scrollbar-width:none !important; } #pdf-scroll::-webkit-scrollbar { display:none; }'});
 let loaded=false;
 for(const step of workflow) {
  mark('phase',{name:step.name});
  const started=Date.now();
  if(step.page>0) {
   if(!loaded) {
    await page.locator('#file-input').setInputFiles(source);
    await page.waitForFunction(()=>__diag.spans.some(s=>s.name==='handleSelectedPDFs'&&s.end),{},{timeout:45000});
    loaded=true;
   }
   await within(page.evaluate(async(step)=>{
    msrSnapEnabled=false;
    const pg=await pdfDoc.getPage(step.page);
    const vp=getPageViewport(pg,step.page,1);
    const nextScale=900*0.95/vp.width*step.zoom;
    if(currentPage!==step.page || Math.abs(scale-nextScale)>1e-9) {
      scale=nextScale;
      recenterAfterRender=true;
      await renderPage(step.page);
    }
    pdfScroll.scrollLeft=canvasPadX+(canvas.width-900)/2-step.pan[0]*canvas.width;
    pdfScroll.scrollTop=canvasPadY+(canvas.height-650)/2-step.pan[1]*canvas.height;
   },step),60000);
  }
  if(errors.length)throw new Error(errors.join('; '));
  mark('ready',{name:step.name,viewport:await page.evaluate(()=>({width:pdfScroll.clientWidth,height:pdfScroll.clientHeight,canvas:[canvas.width,canvas.height]}))});
  await pause(step.dwell*1000);
  mark('settled',{name:step.name,elapsed_ms:Date.now()-started});
 }
 mark('complete',{success:true});
} catch(e) { mark('failed',{message:String(e)}); }
finally {
 if(browser)await browser.close().catch(()=>{});
 if(browserServer)await browserServer.close().catch(()=>{});
 await new Promise(resolve=>server.close(resolve));
}
