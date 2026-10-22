// Optional release packaging; run from the repository root. Requires zip.
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
const root=process.cwd();
if(path.basename(root)!=='sentinelflow-ids')throw new Error('Run from sentinelflow-ids root');
const skip=new Set(['target','node_modules','dist','test-results','playwright-report','.git','demo','workload.jsonl','CHECKPOINT.md','project-specification.md','validation-status.json']);
const files=[];
function walk(dir){for(const e of fs.readdirSync(dir,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))){if(skip.has(e.name))continue;const p=path.join(dir,e.name);if(e.isSymbolicLink())throw new Error('Unexpected symlink: '+p);if(e.isDirectory())walk(p);else if(e.isFile())files.push(path.relative(path.dirname(root),p));}}
walk(root);
const destination=path.resolve(process.argv[2]??'../sentinelflow-ids.zip');
if(fs.existsSync(destination))throw new Error('Archive already exists; choose a new path');
const result=spawnSync('zip',['-q',destination,'-@'],{cwd:path.dirname(root),input:files.join('\n')+'\n',encoding:'utf8'});
if(result.status!==0)throw new Error(result.stderr);
console.log(`${files.length} files packaged in ${destination}`);
