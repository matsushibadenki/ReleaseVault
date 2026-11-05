export type Scope={project:number,target:string,env:string}
export type Def={name:string,key:string,group:string,guide:string,kind:string,targets:string[]}
export type Row={def:Def,present:boolean,display:string|null,expires:string|null,synced:string|null,binding?:{automatic:boolean,polled:string,remote:string}|null,evidence:string|null}
export type Project={id:number,name:string,targets:string[]}
export type Audit={at:string,action:string,env:string,key:string}
export type Sheet={locked:false,rows:Row[],projects:Project[],scope:Scope,selected:string,floating:boolean,audit:Audit[]}
export type Snapshot=Sheet|{locked:true,exists:boolean}
export type T=(ja:string,en:string,zh:string)=>string
export function status(row:Row){const today=new Date().toISOString().slice(0,10);return !row.present?'missing':row.expires&&row.expires<today?'expired':row.expires&&Date.parse(row.expires)-Date.parse(today)<=30*86400000?'soon':'configured'}
