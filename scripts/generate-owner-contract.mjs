// Rust serde DTOs are the source of the owner transport schema.
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const contract=JSON.parse(execFileSync('cargo',['run','--quiet','--locked','-p','conn-web','--','contract'],{cwd:root,encoding:'utf8'}));
const schemas=new Map();
for(const schema of Object.values(contract)){if(schema&&typeof schema==='object'&&schema.title){for(const [name,definition]of Object.entries(schema.definitions??{}))schemas.set(name,definition);schemas.set(schema.title,schema);}}
function type(schema){
 if(schema.$ref)return schema.$ref.split('/').at(-1);
 if(schema.enum)return schema.enum.map(value=>JSON.stringify(value)).join(' | ');
 if(schema.const!==undefined)return JSON.stringify(schema.const);
 if(schema.anyOf||schema.oneOf)return (schema.anyOf??schema.oneOf).map(type).join(' | ');
 if(schema.allOf)return schema.allOf.map(type).join(' & ');
 if(Array.isArray(schema.type))return schema.type.map(t=>type({...schema,type:t})).join(' | ');
 switch(schema.type){
  case 'null':return 'null';case 'boolean':return 'boolean';case 'integer':case 'number':return 'number';case 'string':return 'string';
  case 'array':return `Array<${type(schema.items??{})}>`;
  case 'object':{
   const properties=Object.entries(schema.properties??{}).map(([name,value])=>`${JSON.stringify(name)}${schema.required?.includes(name)?'':'?'}: ${type(value)};`);
   if(schema.additionalProperties&&typeof schema.additionalProperties==='object')properties.push(`[key: string]: ${type(schema.additionalProperties)};`);
   return properties.length?`{\n${properties.map(p=>'  '+p).join('\n')}\n}`:'Record<string, unknown>';
  }
  default:return 'unknown';
 }
}
const text='// Generated from conn-frontend owner DTOs. Do not edit.\n// Run: node scripts/generate-owner-contract.mjs [--check]\n\n'+`export const OWNER_PROTOCOL = ${contract.protocol} as const;\n\n`+[...schemas].sort(([a],[b])=>a.localeCompare(b)).map(([name,schema])=>`export type ${name} = ${type(schema)};\n`).join('\n');
const target=resolve(root,'packages/ui/src/runtime/generated.ts');
if(process.argv.includes('--check')){if(readFileSync(target,'utf8')!==text)throw Error('Owner wire types differ from Rust. Run node scripts/generate-owner-contract.mjs.');console.log('Owner wire contract matches Rust.');}else{writeFileSync(target,text);console.log('Generated packages/ui/src/runtime/generated.ts');}
