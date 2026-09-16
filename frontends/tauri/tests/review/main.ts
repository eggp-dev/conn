// Isolated visual regression fixture; never imported by the application build.
import { mockIPC } from '@tauri-apps/api/mocks';
import { mount } from 'svelte';
import Fixture from './Fixture.svelte';
import '../../src/app.css';
import { st, newTab } from '../../src/lib/store.svelte';
import { recordTimeline } from '../../src/lib/timeline';
import { setLang } from '../../src/lib/i18n.svelte';

const options = new URLSearchParams(location.search);
setLang(options.get('lang') === 'ko' ? 'ko' : 'en');
st.active = 't1'; st.order = ['t1']; st.tabs.t1 = newTab('t1', 1);
st.settingsTab = 'diagnostics'; st.timelineOpen = true;
if (options.has('automation')) { st.settingsTab = 'automation'; st.settingsOpen = true; st.timelineOpen = false; }
let automationConfig = {enabled:false,profiles:['local-zsh']};
let requiresReenable = options.has('reenable');
let automationReads = 0;
const automationInfo = () => ({config:structuredClone(automationConfig),nativeSupported:!options.has('unsupported'),requiresReenable});
const s = st.tabs.t1.timeline;
const events = [
  {event:'control_granted',agentId:'codex',lease:'l1'},
  {event:'agent_exec',agentId:'codex',cmd:'pwd',policy:'allow'},
  {event:'exec_scheduled',execId:'e1',agentId:'codex',cmd:'ls',graceMs:5000},
  {event:'exec_cosigned',execId:'e1',agentId:'codex'},
  {event:'agent_exec',agentId:'codex',cmd:'ls',policy:'allow'},
  {event:'proposal_changed',proposal:{proposalId:'p1',agentId:'codex',text:'mkdir sample',state:'ready'}},
  {event:'proposal_resolved',proposalId:'p1',state:'executed'},
  {event:'agent_exec',agentId:'codex',cmd:'mkdir sample',policy:'allow'},
  {event:'approval_requested',request:{id:'a1',agentId:'codex',cmd:'rm sample',label:'delete'}},
  {event:'approval_resolved',approvalId:'a1',state:'granted',by:'human'},
  {event:'agent_exec',agentId:'codex',cmd:'rm sample',policy:'confirm:granted'},
];
events.forEach((e,i) => recordTimeline(s,e,Date.now()+i));
mockIPC((name,args:any) => {
  if (name === 'dispatch') { name = args.name; args = args.args; }
  if (name === 'automation_settings') {
    const snapshot = automationInfo();
    automationReads++;
    return options.has('slowPoll') && automationReads === 2 ? new Promise(resolve => setTimeout(() => resolve(snapshot),1000)) : snapshot;
  }
  if (name === 'automation_save') {
    if(options.has('failSave')) throw new Error('Fixture: unable to save settings');
    // Native IPC serializes Svelte proxies before Rust receives them.
    automationConfig = JSON.parse(JSON.stringify(args.config)); requiresReenable = false; return automationInfo();
  }
  if (name === 'automation_revoke') {
    if(options.has('failRevoke')) throw new Error('Fixture: unable to revoke access');
    return null;
  }
  if (name === 'profiles_catalog') return {config:{profiles:[{id:'local-zsh',name:'Local zsh'},{id:'ssh-development',name:'Development SSH connection with a long profile name'}]}};
  if (name === 'diagnostics') return {
    version:'test',sessions:2,socket:'/test/conn.sock',configDir:'/test/config',policyPath:'/test/config/policy.yaml',auditPath:'/test/config/audit.jsonl',
    cli:{onPath:'/test/conn',canInstall:false,canConfigure:false},
    clients:[{id:'copilot-cli',name:'Copilot CLI',state:'not_configured'},{id:'codex',name:'Codex CLI',state:'configured'}],
    agentConnections:[{connId:1,agentId:'copilot',idleSecs:720,sessions:['t1']},{connId:2,agentId:'copilot',idleSecs:1,sessions:['t1','t2']}],
  };
  if (name === 'set_mode') throw new Error('shell input is pending; clear or cancel it before changing mode');
  return null;
},{shouldMockEvents:true});
mount(Fixture,{target:document.getElementById('app')!});
