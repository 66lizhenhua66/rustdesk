(() => {
  'use strict';
  const active = state => ['view', 'control'].includes(state.session);
  const pending = ['running', 'paused', 'conflict'];
  const labels = { running: '传输中', paused: '已暂停', conflict: '待处理同名文件', completed: '已完成', failed: '传输失败', cancelled: '已取消', stopped: '已停止' };
  const seed = {
    local: [
      { id: 'local-pdf', name: '项目说明.pdf', size: '1.2 MB', kind: 'PDF', folder: 'documents' },
      { id: 'local-zip', name: '设计稿.zip', size: '24.6 MB', kind: 'ZIP', folder: 'documents' },
      { id: 'local-txt', name: '会议纪要.txt', size: '8 KB', kind: 'TXT', folder: 'documents' },
      { id: 'local-project', name: '交接清单.txt', size: '4 KB', kind: 'TXT', folder: 'project' },
    ],
    remote: [
      { id: 'remote-xlsx', name: '报表.xlsx', size: '320 KB', kind: 'XLSX', folder: 'documents' },
      { id: 'remote-pdf', name: '项目说明.pdf', size: '960 KB', kind: 'PDF', folder: 'documents' },
      { id: 'remote-project', name: '项目排期.xlsx', size: '180 KB', kind: 'XLSX', folder: 'project' },
    ],
  };
  function init(state) {
    state.filePermissions ||= { upload: false, download: false };
    state.fileRequests ||= { upload: false, download: false };
    state.fileTransfers ||= [];
    state.fileSelections ||= { local: [], remote: [] };
    state.fileFolder ||= { local: 'documents', remote: 'documents' };
    state.fileTab ||= 'local';
    state.fileEntries ||= { local: seed.local.map(f => ({ ...f })), remote: seed.remote.map(f => ({ ...f })) };
    state.fileCounter ||= 0;
  }
  const path = (state, side) => `${side === 'local' ? '本机文档' : '远程共享'}${state.fileFolder[side] === 'project' ? ' / 项目资料' : ''}`;
  function button(action, label, cls = '', disabled = false) {
    return `<button type="button" class="file-btn ${cls}" data-file-action="${action}"${disabled ? ' disabled' : ''}>${label}</button>`;
  }
  function permission(state, direction, h) {
    const enabled = state.filePermissions[direction];
    const requested = state.fileRequests[direction];
    const name = direction === 'upload' ? '上传到远端' : '下载到本机';
    return `<div class="file-permission ${enabled ? 'allowed' : ''}">${h.icon(enabled ? 'check' : 'lock', 16)}<span><b>${name}</b><small>${enabled ? '本次会话已允许' : requested ? '等待被控端允许' : '需要单独授权'}</small></span>${enabled ? '<span class="file-status-chip">已允许</span>' : button(`request-${direction}`, requested ? '已请求' : `请求${direction === 'upload' ? '上传' : '下载'}`, '', requested || !active(state))}</div>`;
  }
  function pane(state, side, h) {
    const local = side === 'local';
    const readable = local || (state.filePermissions.download && active(state));
    const direction = local ? 'upload' : 'download';
    const selected = state.fileSelections[side];
    const entries = state.fileEntries[side].filter(f => f.folder === state.fileFolder[side]);
    const device = h.sessionDevice ? h.sessionDevice().name : '工作室电脑';
    return `<section class="file-pane ${state.fileTab === side ? 'is-current' : ''}" aria-label="${local ? '本机文件' : '远端文件'}"><header><div class="file-device-icon">${h.icon(local ? 'folder' : 'monitor', 20)}</div><div><h3>${local ? '本机' : h.escape(device)}</h3><p>${local ? '发送文件的来源' : '接收或获取文件的设备'}</p></div><span class="file-side-label">${local ? 'LOCAL' : 'REMOTE'}</span></header><div class="file-path">${h.icon('folder', 16)}<span>${h.escape(path(state, side))}</span>${state.fileFolder[side] !== 'documents' && readable ? button(`folder:${side}:documents`, '返回上级', 'subtle') : ''}</div>${readable ? `<div class="file-list"><div class="file-list-label"><span>名称</span><span>大小</span></div>${state.fileFolder[side] === 'documents' ? `<button type="button" class="file-row file-folder" data-file-action="folder:${side}:project"><span class="file-type folder-kind">${h.icon('folder', 20)}</span><span class="file-name"><b>项目资料</b><small>文件夹</small></span><span class="file-row-tail">${h.icon('right', 15)}</span></button>` : ''}${entries.map(f => `<button type="button" class="file-row ${selected.includes(f.id) ? 'selected' : ''}" data-file-action="select:${side}:${f.id}" aria-pressed="${selected.includes(f.id)}"><span class="file-check">${selected.includes(f.id) ? h.icon('check', 13) : ''}</span><span class="file-type ${f.kind.toLowerCase()}">${h.icon('file', 20)}</span><span class="file-name"><b>${h.escape(f.name)}</b><small>${h.escape(f.kind)} 文件</small></span><span class="file-size">${h.escape(f.size)}</span></button>`).join('') || '<p class="file-empty">这个示例文件夹还没有文件</p>'}</div>` : `<div class="file-locked">${h.icon('lock', 27)}<h4>远端文件尚未开放</h4><p>对方允许下载后，才会展示远端目录和文件。上传授权不开放文件浏览。</p>${button('request-download', state.fileRequests.download ? '已请求，等待允许' : '请求下载与浏览', '', state.fileRequests.download || !active(state))}</div>`}<footer><span>${readable ? `已选 ${selected.length} 个文件` : '远端目录受保护'}</span>${button(`send-${direction}`, local ? '上传到远端 →' : '← 下载到本机', 'primary', !active(state) || !state.filePermissions[direction] || !selected.length)}</footer></section>`;
  }
  function transfer(task, h) {
    const direction = task.direction === 'upload' ? '本机 → 远端' : '远端 → 本机';
    const action = (name, label, cls = '') => button(`${name}:${task.id}`, label, cls);
    let controls = '';
    if (task.status === 'running') controls = action('pause', '暂停') + action('cancel', '取消', 'subtle');
    if (task.status === 'paused') controls = action('resume', '继续', 'primary') + action('cancel', '取消', 'subtle');
    if (task.status === 'failed') controls = action('retry', '重新传输');
    const conflict = task.status === 'conflict' ? `<div class="file-conflict"><b>目标目录已有「${h.escape(task.name)}」</b><p>请选择这一个任务的处理方式。仅改变原型中的模拟文件。</p><div>${task.confirmReplace ? `${action('confirm-replace', '确认替换此模拟文件', 'danger')}${action('back-conflict', '返回选择')}` : `${action('keep-both', '保留两份', 'primary')}${action('skip', '跳过')}${action('replace', '替换…', 'danger')}`}</div></div>` : '';
    return `<article class="file-task ${task.status}"><div class="file-task-top"><div class="file-type">${h.icon('file', 18)}</div><div class="file-task-name"><b>${h.escape(task.name)}</b><small>${direction} · ${h.escape(task.size)}</small></div><span class="file-task-status">${labels[task.status]}</span></div><div class="file-task-progress"><div role="progressbar" aria-label="${h.escape(task.name)} 传输进度" aria-valuenow="${task.progress}" aria-valuemin="0" aria-valuemax="100"><i style="width:${task.progress}%"></i></div><span>${task.progress}%</span></div><div class="file-task-bottom"><span>${task.reason ? h.escape(task.reason) : task.status === 'completed' ? '已加入目标模拟目录' : `目标：${task.direction === 'upload' ? '远程共享' : '本机文档'}${task.targetFolder === 'project' ? ' / 项目资料' : ''}`}</span><div>${controls}</div></div>${conflict}${task.status === 'running' ? `<div class="file-task-simulation"><span>模拟事件</span>${action('advance', '推进传输')}${action('fail', '模拟失败')}${action('conflict', '同名冲突')}</div>` : ''}</article>`;
  }
  function render(state, h) {
    init(state);
    const running = state.fileTransfers.filter(t => pending.includes(t.status)).length;
    return `<div class="file-root"><header class="file-header"><div><span class="file-eyebrow">FILES BETWEEN DEVICES</span><h2>文件传输</h2><p>桌面保持连接，文件按需往返。</p></div><button type="button" class="icon-btn" data-action="close-files" aria-label="关闭文件传输，返回桌面">${h.icon('close', 20)}</button></header><div class="file-permissions">${permission(state, 'upload', h)}${permission(state, 'download', h)}</div><div class="file-mode-note">${h.icon('shield', 15)}<span>${state.session === 'control' ? '控制模式' : '仅查看模式'}下，上传、下载仍需各自授权；撤销文件权限会停止传输。</span></div>${!active(state) ? '<div class="file-disconnected" role="status">连接已结束。传输已停止，请重新连接并获得文件权限。</div>' : ''}<div class="file-tabs" role="tablist" aria-label="文件位置"><button role="tab" type="button" data-file-action="tab-local" aria-selected="${state.fileTab === 'local'}" class="${state.fileTab === 'local' ? 'active' : ''}">本机 · 上传</button><button role="tab" type="button" data-file-action="tab-remote" aria-selected="${state.fileTab === 'remote'}" class="${state.fileTab === 'remote' ? 'active' : ''}">远端 · 下载</button></div><div class="file-panes">${pane(state, 'local', h)}${pane(state, 'remote', h)}</div><section class="file-queue"><div class="file-queue-heading"><h3>传输任务 <span>${state.fileTransfers.length}</span></h3><p>${running ? `${running} 个任务处理中` : '每个任务独立展示结果'}</p></div>${state.fileTransfers.length ? `<div class="file-task-list">${state.fileTransfers.map(t => transfer(t, h)).join('')}</div>` : `<div class="file-queue-empty">${h.icon('folder', 22)}<div><b>选好文件，再选择传输方向</b><p>进度、暂停、失败和同名处理会显示在这里。</p></div></div>`}</section><details class="file-demo"><summary>原型演示 · 授权与事件</summary><p>文件、目录与进度均为内存模拟，不会读取或写入真实设备。点击任务中的「推进传输」可体验完成。</p><div>${['upload', 'download'].map(d => button(state.filePermissions[d] ? `demo-revoke-${d}` : `demo-grant-${d}`, state.filePermissions[d] ? `对方撤销${d === 'upload' ? '上传' : '下载'}` : `对方允许${d === 'upload' ? '上传' : '下载'}`, '', !active(state))).join('')}</div></details></div>`;
  }
  function stop(state, reason = '会话或文件授权已结束', direction) {
    init(state);
    state.fileTransfers.forEach(task => {
      if ((pending.includes(task.status) || task.status === 'failed') && (!direction || task.direction === direction)) { task.status = 'stopped'; task.reason = reason; task.confirmReplace = false; }
    });
    if (direction) {
      state.fileSelections[direction === 'upload' ? 'local' : 'remote'] = [];
      state.fileRequests[direction] = false;
    } else {
      state.fileSelections = { local: [], remote: [] };
      state.fileRequests = { upload: false, download: false };
    }
  }
  function canTransfer(state, direction, h) {
    if (!active(state)) { h.notify('连接已结束，请重新连接后再传输'); return false; }
    if (!state.filePermissions[direction]) { h.notify(`尚未获得${direction === 'upload' ? '上传' : '下载'}权限`); return false; }
    return true;
  }
  function targetEntries(state, task) { return state.fileEntries[task.direction === 'upload' ? 'remote' : 'local']; }
  function collision(state, task) { return targetEntries(state, task).find(f => f.folder === task.targetFolder && f.name === task.name); }
  function complete(state, task) {
    const existing = collision(state, task);
    if (existing && !task.replaceConfirmed) { task.status = 'conflict'; task.progress = Math.min(task.progress, 95); return; }
    const entries = targetEntries(state, task);
    const next = { id: `received-${task.id}`, name: task.name, kind: task.kind, size: task.size, folder: task.targetFolder };
    if (existing) entries.splice(entries.indexOf(existing), 1, next); else entries.push(next);
    task.status = 'completed'; task.progress = 100; task.reason = '';
  }
  function handle(action, state, h) {
    init(state);
    if (action.startsWith('tab-')) { state.fileTab = action.slice(4) === 'remote' ? 'remote' : 'local'; h.render(); return true; }
    if (action.startsWith('request-')) {
      const d = action.slice(8); if (!['upload', 'download'].includes(d)) return false;
      if (!active(state)) { h.notify('请先连接远端设备'); return true; }
      state.fileRequests[d] = true; h.notify(`已请求${d === 'upload' ? '上传' : '下载'}权限，等待被控端允许`); h.render(); return true;
    }
    if (action.startsWith('demo-grant-') || action.startsWith('demo-revoke-')) {
      const d = action.split('-').at(-1); if (!['upload', 'download'].includes(d)) return false;
      if (!active(state)) { h.notify('会话已结束，不能授予文件权限'); return true; }
      const grant = action.startsWith('demo-grant-');
      state.filePermissions[d] = grant; state.fileRequests[d] = false;
      if (!grant) stop(state, `对方撤销了${d === 'upload' ? '上传' : '下载'}权限`, d);
      h.notify(`模拟：对方已${grant ? '允许' : '撤销'}${d === 'upload' ? '上传' : '下载'}`); h.render(); return true;
    }
    if (action.startsWith('folder:') || action.startsWith('select:')) {
      const [verb, side, id] = action.split(':');
      if (!['local', 'remote'].includes(side)) return false;
      if (side === 'remote' && !canTransfer(state, 'download', h)) return true;
      if (verb === 'folder') {
        if (!['documents', 'project'].includes(id)) return false;
        state.fileFolder[side] = id; state.fileSelections[side] = [];
      } else {
        const item = state.fileEntries[side].find(f => f.id === id && f.folder === state.fileFolder[side]);
        if (!item) return false;
        const selected = state.fileSelections[side]; state.fileSelections[side] = selected.includes(id) ? selected.filter(x => x !== id) : [...selected, id];
      }
      h.render(); return true;
    }
    if (action.startsWith('send-')) {
      const d = action.slice(5); if (!['upload', 'download'].includes(d)) return false;
      if (!canTransfer(state, d, h)) return true;
      const side = d === 'upload' ? 'local' : 'remote';
      const files = state.fileEntries[side].filter(f => state.fileSelections[side].includes(f.id) && f.folder === state.fileFolder[side]);
      if (!files.length) { h.notify('先选择要传输的文件'); return true; }
      files.forEach(f => {
        const task = { id: `task-${++state.fileCounter}`, direction: d, name: f.name, kind: f.kind, size: f.size, targetFolder: state.fileFolder[d === 'upload' ? 'remote' : 'local'], status: 'running', progress: 0 };
        if (collision(state, task)) task.status = 'conflict';
        state.fileTransfers.unshift(task);
      });
      state.fileSelections[side] = []; h.notify(`已添加 ${files.length} 个模拟传输任务`); h.render(); return true;
    }
    const [verb, id] = action.split(':');
    const task = state.fileTransfers.find(t => t.id === id);
    if (!task) return false;
    if (verb === 'cancel' && pending.includes(task.status)) { task.status = 'cancelled'; task.reason = '已取消，不改变目标文件'; h.render(); return true; }
    if (verb === 'pause' && task.status === 'running') { task.status = 'paused'; h.render(); return true; }
    if (!canTransfer(state, task.direction, h)) return true;
    if (verb === 'resume' && task.status === 'paused') task.status = 'running';
    else if (verb === 'retry' && task.status === 'failed') { task.status = collision(state, task) && !task.replaceConfirmed ? 'conflict' : 'running'; task.progress = 0; task.reason = ''; }
    else if (verb === 'advance' && task.status === 'running') { task.progress = Math.min(100, task.progress + 35); if (task.progress === 100) complete(state, task); }
    else if (verb === 'fail' && task.status === 'running') { task.status = 'failed'; task.reason = '模拟失败：传输暂时不可用，可重新传输'; }
    else if (verb === 'conflict' && task.status === 'running') { task.status = 'conflict'; task.confirmReplace = false; }
    else if (task.status === 'conflict') {
      if (verb === 'skip') { task.status = 'cancelled'; task.reason = '已跳过同名文件'; }
      else if (verb === 'replace') task.confirmReplace = true;
      else if (verb === 'back-conflict') task.confirmReplace = false;
      else if (verb === 'confirm-replace' && task.confirmReplace) { task.replaceConfirmed = true; task.confirmReplace = false; task.status = 'running'; }
      else if (verb === 'keep-both') {
        const split = task.name.lastIndexOf('.'); const base = split > 0 ? task.name.slice(0, split) : task.name; const ext = split > 0 ? task.name.slice(split) : '';
        let suffix = 2;
        do { task.name = `${base} (${suffix++})${ext}`; } while (collision(state, task));
        task.status = 'running'; task.confirmReplace = false;
      } else return false;
    } else return false;
    h.render(); return true;
  }
  window.FilePrototype = { render, handle, stop };
})();
