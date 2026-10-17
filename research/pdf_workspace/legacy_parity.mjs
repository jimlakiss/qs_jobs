// Runs the actual Jobs functions without loading the DOM, and the actual Ruby
// quantity builder without starting Rails. Expected figures are independent.
import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const source = fs.readFileSync(root + 'public/pdf_viewer/pdf_viewer.js', 'utf8');
const names = ['msrNormToMeters','msrNormToSqMeters','msrLengthMeters','msrAreaSqMeters',
  'msrPerimeterMeters','msrPointInPoly','msrFindZone','msrFindSnapPt'];
const extracted = names.map(name => {
  const match = source.match(new RegExp(`^function ${name}\\([^]*?^\\}`, 'm'));
  assert(match, `Could not locate production function ${name}`);
  return match[0];
}).join('\n');
const context = vm.createContext({currentPage:1,scaleZonesByPage:{}, msrSnapEnabled:true,
  canvas:{width:2048,height:2048},snapPointsByPage:new Map(),snapSegmentsByPage:new Map()});
vm.runInContext(extracted, context);
const rect = (a,b,c,d) => [{x:a,y:b},{x:c,y:b},{x:c,y:d},{x:a,y:d}];
const zone = (mpp=100, vertices=rect(0,0,1,1)) => ({mpp, label:`1:${mpp}`, vertices});
const cases = [
  {name:'square: known Rails example',type:'area',points:rect(0,0,.1,.1),zones:[zone()],expected:{area_m2:6.4516,perimeter_m:10.16}},
  {name:'reverse winding',type:'area',points:rect(0,0,.1,.1).reverse(),zones:[zone()],expected:{area_m2:6.4516,perimeter_m:10.16}},
  {name:'open polyline',type:'linear',points:[{x:.1,y:.1},{x:.2,y:.1},{x:.2,y:.3}],zones:[zone()],expected:{length_m:7.62}},
  {name:'newest overlapping zone wins',type:'linear',points:[{x:.1,y:.1},{x:.2,y:.1}],zones:[zone(),zone(50)],expected:{length_m:1.27}},
  {name:'whole polyline uses first point zone',type:'linear',points:[{x:.1,y:.1},{x:.9,y:.1}],zones:[zone(),zone(50,rect(.5,0,1,1))],expected:{length_m:20.32}},
  {name:'area uses vertex mean zone',type:'area',points:rect(.1,.1,.3,.3),zones:[zone(),zone(50,rect(.15,.15,.25,.25))],expected:{area_m2:6.4516,perimeter_m:10.16}},
  {name:'unscaled line remains unquantified',type:'linear',points:[{x:.1,y:.1},{x:.2,y:.1}],zones:[],expected:{}},
  {name:'missing dimensions remains unquantified',type:'area',points:rect(0,0,.1,.1),zones:[zone()],dims:null,expected:{}},
  {name:'count needs no calibration',type:'count',points:[{x:.1,y:.1},{x:.1,y:.1},{x:.2,y:.2}],zones:[],expected:{count:3}},
  {name:'non-square page axis conversion',type:'linear',points:[{x:.1,y:.1},{x:.2,y:.2}],zones:[zone()],dims:{width:720,height:360},expected:{length_m:Math.hypot(2.54,1.27)}},
];
for (const c of cases) if (!Object.hasOwn(c,'dims')) c.dims={width:720,height:720};
const rubyExecutable=process.env.IQS_RUBY || 'ruby';
const ruby = spawnSync(rubyExecutable,['-rjson','-e',`
require ARGV[0]
builder = MeasuredDimensionsBuilder.allocate
puts JSON.generate(JSON.parse(STDIN.read).map { |c|
  builder.send(:measurement_values, c, c['dims'], c['zones'])
})`, root+'app/models/measured_dimensions_builder.rb'],{input:JSON.stringify(cases),encoding:'utf8'});
assert.equal(ruby.status,0,ruby.stderr);
const rubyResults=JSON.parse(ruby.stdout);
const keys=['area_m2','perimeter_m','length_m','count'];
function close(actual,expected,message){
  if(expected==null) assert.equal(actual??null,null,message);
  else assert(Math.abs(actual-expected)<=1e-9*Math.max(1,Math.abs(expected)),`${message}: ${actual} != ${expected}`);
}
for (let i=0;i<cases.length;i++) {
  const c=cases[i]; context.scaleZonesByPage={1:c.zones};
  const js = c.type==='count'?{count:c.points.length}:c.type==='linear'
    ?{length_m:context.msrLengthMeters(c,c.dims)}
    :{area_m2:context.msrAreaSqMeters(c,c.dims),perimeter_m:context.msrPerimeterMeters(c,c.dims)};
  for(const key of keys){close(js[key],c.expected[key],`${c.name}: JS ${key}`);close(rubyResults[i][key],c.expected[key],`${c.name}: Ruby ${key}`);}
}
// Computational baseline only: excludes browser events, index build and paint.
const points=Array.from({length:20000},(_,i)=>({x:(i%200)/200,y:Math.floor(i/200)/100}));
context.snapPointsByPage.set(1,points);
context.snapSegmentsByPage.set(1,points.slice(1).map((p,i)=>({a:points[i],b:p})));
for(let i=0;i<100;i++)context.msrFindSnapPt(.5,.5);
const timings=[];
for(let i=0;i<300;i++){const t=performance.now();context.msrFindSnapPt((i%100)/100,.515);timings.push(performance.now()-t);}
timings.sort((a,b)=>a-b);
console.log(JSON.stringify({kind:'legacy_parity_and_snap',cases:cases.length,quantity_tolerance:1e-9,
  production_source:'public/pdf_viewer/pdf_viewer.js + app/models/measured_dimensions_builder.rb',
  node:process.version,ruby:spawnSync(rubyExecutable,['--version'],{encoding:'utf8'}).stdout.trim(),platform:process.platform,arch:process.arch,
  snap:{points:points.length,segments:points.length-1,samples:timings.length,
    p50_ms:timings[149],p95_ms:timings[284],p99_ms:timings[296],
    method:'Node VM; linear scan; synthetic geometry; no browser/rendering/input latency'}},null,2));
