"""本地模拟宿主交互回归；需要 Python Playwright；使用临时本地 HTTP 服务。
Mock-host WebUI regression; never connects to a device.
"""
import re
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread
from pathlib import Path
from playwright.sync_api import sync_playwright,expect
MOCK=r"""
localStorage.setItem('tf-lang', 'zh');
window.mock = {commands: [], failNext: false, pending: null};
window.ksu = {exec(command, options, callback) {
 window.mock.commands.push(command);
 const saved = JSON.parse(localStorage.getItem('test-config') || '{"keybox_source":"remote","keybox_remote_slot":1}');
 let data;
 if (command.includes('--bootstrap')) data={device:{brand:'Test',model:'Synthetic',name:'Test',android:'16',sdk:'36',security_patch:'',abi:'arm64-v8a',soc_model:'',soc_manufacturer:'',verified_boot:''},system:{kernel:'test',selinux:'Enforcing'},module:{version:'test',version_code:1},root:{method:'KernelSU',version:'test'},config:{...saved,target_mode:'auto',update_channel:'dev'},capabilities:{rkp_artifacts:true,rkp_supported:false,rkp_platform:true},paths:{rkp_dir:'/test/rkp',keybox_dir:'/test/keybox'}};
 else if (command.includes('--rkp-artifacts')) data={exists:true};
 else if (command.includes('--keybox')) {
  window.mock.pending=()=>{
   if(window.mock.failNext){window.mock.failNext=false;window[callback](1,JSON.stringify({ok:false,error:{message:'synthetic download failure'}}),'');return;}
   const source=command.includes('--source rkp')?'rkp':'remote';
   const slot=Number(command.match(/--remote-slot (\d)/)?.[1]||saved.keybox_remote_slot);
   localStorage.setItem('test-config',JSON.stringify({keybox_source:source,keybox_remote_slot:slot}));
   window[callback](0,JSON.stringify({ok:true,data:{source,remote_slot:source==='remote'?slot:null,targets:[{path:'/test/local',changed:command.includes('--force')},{path:'/test/backend',changed:true}]}}),'');
  };
  return;
 }
 else {window[callback](1,'{}','unexpected mock command');return;}
 setTimeout(()=>window[callback](0,JSON.stringify({ok:true,data}),''),0);
}};
"""
server = ThreadingHTTPServer(('127.0.0.1', 0), partial(SimpleHTTPRequestHandler, directory=str(Path(__file__).resolve().parents[1] / 'module/webroot')))
thread = Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    with sync_playwright() as p:
     browser=p.chromium.launch(headless=True)
     page=browser.new_page(viewport={"width":390,"height":844})
     errors=[]
     page.on('pageerror',lambda e:errors.append(str(e)))
     page.add_init_script(MOCK)
     page.goto(f'http://127.0.0.1:{server.server_port}/#/keybox')
     page.wait_for_load_state('networkidle')
     slot=lambda n:page.get_by_role('button',name=f'槽位 {n}',exact=True)
     expect(slot(1)).to_have_attribute('aria-pressed','true')
     slot(2).click()
     page.get_by_role('button',name='应用所选来源',exact=True).click()
     expect(page.get_by_role('dialog')).to_contain_text('槽位 2')
     page.evaluate('window.mock.failNext=true')
     page.get_by_role('button',name='确认',exact=True).click()
     expect(slot(1)).to_be_disabled()
     expect(page.get_by_role('button',name='取消',exact=True)).to_be_disabled()
     page.keyboard.press('Escape')
     expect(page.get_by_role('dialog')).to_be_visible()
     assert '--remote-slot 2' in page.evaluate('window.mock.commands.at(-1)')
     assert '--force' not in page.evaluate('window.mock.commands.at(-1)')
     page.evaluate('window.mock.pending()')
     expect(page.get_by_role('dialog')).not_to_be_visible()
     expect(page.get_by_text('synthetic download failure',exact=True)).to_be_visible()
     assert page.evaluate("JSON.parse(localStorage.getItem('test-config')||'{}').keybox_remote_slot") is None
     page.get_by_role('button',name='应用所选来源',exact=True).click()
     page.get_by_role('button',name='确认',exact=True).click()
     page.evaluate('window.mock.pending()')
     expect(page.get_by_role('dialog')).not_to_be_visible()
     expect(page.get_by_text('操作成功，实际写入 1 个目标',exact=True)).to_be_visible()
     assert page.evaluate("JSON.parse(localStorage.getItem('test-config')).keybox_remote_slot")==2
     page.reload();page.wait_for_load_state('networkidle')
     expect(slot(2)).to_have_attribute('aria-pressed','true')
     slot(3).click()
     page.get_by_role('button',name='强制拉取并替换',exact=True).click()
     expect(page.get_by_role('dialog')).to_contain_text('即使内容与本地相同')
     page.get_by_role('button',name='确认',exact=True).click()
     command=page.evaluate('window.mock.commands.at(-1)')
     assert '--remote-slot 3' in command and '--force' in command and '--save-selection' in command
     page.evaluate('window.mock.pending()')
     expect(page.get_by_text('操作成功，实际写入 2 个目标',exact=True)).to_be_visible()
     Path('out').mkdir(exist_ok=True)
     page.screenshot(path='out/keybox-slots-remote.png',full_page=True)
     page.get_by_role('button',name=re.compile('本机 RKP 签发')).click()
     expect(slot(3)).not_to_be_visible()
     expect(page.get_by_role('button',name='强制拉取并替换',exact=True)).not_to_be_visible()
     page.get_by_role('button',name='应用所选来源',exact=True).click()
     page.get_by_role('button',name='确认',exact=True).click()
     command=page.evaluate('window.mock.commands.at(-1)')
     assert '--source rkp' in command and '--remote-slot' not in command and '--force' not in command
     page.evaluate('window.mock.pending()')
     assert page.evaluate("JSON.parse(localStorage.getItem('test-config')).keybox_remote_slot")==3
     page.get_by_role('button',name=re.compile('远程公共密钥池')).click()
     expect(slot(3)).to_have_attribute('aria-pressed','true')
     assert not errors,errors
     assert page.locator('[data-render-error]').count()==0
     browser.close()
     print('WebUI mocked-host checks passed: default/slots/normal/force/failure/save/reload/busy/RKP; no runtime errors.')
finally:
    server.shutdown()
    server.server_close()
    thread.join()
