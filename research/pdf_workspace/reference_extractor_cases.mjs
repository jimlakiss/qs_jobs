// Generate an executable reference from the existing, working Jobs source.
import fs from 'node:fs';
import vm from 'node:vm';
const source=fs.readFileSync(new URL('../../public/pdf_viewer/pdf_splitter.js',import.meta.url),'utf8');
function extract(name) {const start=source.indexOf(`  function ${name}(`);if(start<0)throw Error(name);const next=source.indexOf('\n  function ',start+1);return source.slice(start,next<0?undefined:next);}
const sandbox={window:{documentDetails:{}},useSeqPrefix:true,seqSeparator:' - '};
vm.createContext(sandbox);
for(const name of ['sanitize','generateFilename','getStatus'])vm.runInContext(extract(name),sandbox);
const cases=[
 {sheet:{page:1,sheet_id:'A101',description:'Ground floor',issue_id:'B'},template:'{sheet_id} - {description} - {issue_id}',sequence:true,separator:' - ',index:0},
 {sheet:{page:7,sheet_id:'A-102',description:'North / courtyard',issue_id:''},template:'{sheet_id} - {description} - {issue_id}',sequence:true,separator:'_',index:11},
 {sheet:{page:3,sheet_id:'',description:'',issue_id:''},template:'{sheet_id} - {description} - {issue_id}',sequence:false,separator:' - ',index:0},
 {sheet:{page:2,sheet_id:'A201',description:'Elevations',issue_id:''},template:'{project_id} - {sheet_id} - {description} - {issue_id}',sequence:false,separator:' - ',index:1,document:{project_id:'23004'}},
 {sheet:{page:2,sheet_id:'A201',description:'',issue_id:'C'},template:'{sheet_id}_{description}_{issue_id}',sequence:true,separator:' ',index:3},
];
for(const item of cases){sandbox.useSeqPrefix=item.sequence;sandbox.seqSeparator=item.separator;sandbox.window.documentDetails=item.document||{};item.filename=sandbox.generateFilename(item.sheet,item.template,item.index);item.status=sandbox.getStatus(item.sheet);}
fs.writeFileSync(new URL('fixtures/extractor_reference.json',import.meta.url),JSON.stringify(cases,null,2)+'\n');
console.log(`Generated ${cases.length} cases directly from Jobs filename/status functions`);
