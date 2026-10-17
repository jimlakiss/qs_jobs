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
    '<script>window.qsJobsDocument={documentCategory:"extracted_document",navigationDocuments:[]};</script>')
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
const report={kind:'jobs_browser_pdf_diagnostic',pdfjs:modern?require(path.join(modules,'pdfjs-dist/package.json')).version:'3.11.174',skip_thumbnails:skipThumbs,source_basename:path.basename(source),
  source_bytes:fs.statSync(source).size,method:'actual viewer HTML and JS; ERB host config replaced; local pinned assets; no Rails/API writes; fresh headless Chrome profile; default DPI 136',stages:[],errors:[]};
let browser;let page;let stage='launch';
const bounded=(promise,ms,label)=>{
 let timer;return Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error(label+' exceeded '+ms+'ms')),ms);})]).finally(()=>clearTimeout(timer));
};
try{
 browser=await chromium.launch({headless:true,executablePath:process.env.IQS_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
 report.browser=browser.version();
 const context=await browser.newContext({viewport:{width:1440,height:1000},deviceScaleFactor:2});
 await context.route('**/*',route=>route.request().url().startsWith(address+'/')?route.continue():route.abort());
 page=await context.newPage();
 let rejectError;const pageError=new Promise((_,reject)=>{rejectError=reject;});
 page.on('pageerror',e=>{report.errors.push(e.stack||String(e));rejectError(e);});
 page.on('dialog',dialog=>dialog.accept());
 stage='load viewer';await page.goto(address,{waitUntil:'networkidle'});
 stage='initial load including thumbnails';
 if(rawPage){
  stage='minimal PDF.js page 2 render';await page.locator('#file-input').setInputFiles(source);
  report.method='minimal PDF.js 3.11.174 renderer; no Jobs code, thumbnails, snapping or prior page renders';
  const result=await bounded(Promise.race([page.evaluate(async()=>{
   const data=new Uint8Array(await document.getElementById('file-input').files[0].arrayBuffer());
   const doc=await pdfjsLib.getDocument({data,cMapUrl:'/assets/cmaps/',cMapPacked:true,standardFontDataUrl:'/assets/standard_fonts/'}).promise;
   const pg=await doc.getPage(2);const viewport=pg.getViewport({scale:136/72});
   const c=document.getElementById('pdf-canvas');c.width=viewport.width;c.height=viewport.height;
   const start=performance.now();await pg.render({canvasContext:c.getContext('2d'),viewport}).promise;
   return {page:2,render_ms:performance.now()-start};
  }),pageError]),20000,stage);
  report.stages.push(result);report.success=true;
 }else{
 const started=Date.now();await page.locator('#file-input').setInputFiles(source);
 await Promise.race([page.waitForFunction(()=>__diag.spans.some(s=>s.name==='handleSelectedPDFs' && s.end),{},{timeout:20000}),pageError]);
 report.initial_load_ms=Date.now()-started;
 const info=await page.evaluate(()=>({pages:pdfDoc.numPages,screenDPI,scale,width:canvas.width,height:canvas.height}));
 report.info=info;
 for(let p=1;p<=info.pages;p++){
  stage='render page '+p;
  const sample=await bounded(page.evaluate(async(p)=>{
   msrSnapEnabled=false;const start=performance.now();await renderPage(p);
   return {page:p,render_ms:performance.now()-start,width:canvas.width,height:canvas.height,
    two_canvas_rgba_bytes:canvas.width*canvas.height*4*2};
  },p),20000,stage);
  stage='operator inspection page '+p;
  sample.operators=await bounded(page.evaluate(async(p)=>{
   const pg=await pdfDoc.getPage(p);const start=performance.now();const ops=await pg.getOperatorList();
   let paths=0,maxPathOps=0,maxPathArgs=0,totalPathOps=0;
   for(let i=0;i<ops.fnArray.length;i++)if(ops.fnArray[i]===pdfjsLib.OPS.constructPath){
    paths++;const a=ops.argsArray[i];const n=a?.[0]?.length||0;totalPathOps+=n;
    maxPathOps=Math.max(maxPathOps,n);maxPathArgs=Math.max(maxPathArgs,a?.[1]?.length||0);
   }
   return {count:ops.fnArray.length,paths,maxPathOps,maxPathArgs,totalPathOps,inspection_ms:performance.now()-start};
  },p),20000,stage);
  stage='snap cache page '+p;
  sample.snap=await bounded(page.evaluate(async(p)=>{
   msrSnapEnabled=true;const start=performance.now();await msrBuildSnapCache(p);
   const build_ms=performance.now()-start;const queries=[];
   for(let i=0;i<20;i++){const t=performance.now();msrFindSnapPt((i+0.5)/20,0.5);queries.push(performance.now()-t);}
   queries.sort((a,b)=>a-b);msrSnapEnabled=false;
   return {build_ms,points:snapPointsByPage.get(p)?.length,segments:snapSegmentsByPage.get(p)?.length,
    query_p95_ms:queries[18],query_max_ms:queries[19]};
  },p),20000,stage);
  report.stages.push(sample);console.log(JSON.stringify({page:p,render_ms:sample.render_ms,snap:sample.snap}));
 }
 stage='screenshot';await page.screenshot({path:path.join(root,`results/structural-jobs-browser-${label}.png`)});
 report.success=true;
 }
}catch(error){report.success=false;report.failed_stage=stage;report.failure=String(error);console.error(stage+': '+error);}
finally{
 if(page){try{report.instrumentation=await bounded(page.evaluate(()=>__diag),3000,'collect telemetry');}catch(e){report.telemetry_failure=String(e);}}
 if(browser)await bounded(browser.close(),5000,'close browser').catch(()=>{});
 await new Promise(resolve=>server.close(resolve));
 fs.writeFileSync(path.join(root,`results/structural-jobs-browser-${label}.json`),JSON.stringify(report,null,2)+'\n');
 console.log(`Saved results/structural-jobs-browser-${label}.json`);
}
