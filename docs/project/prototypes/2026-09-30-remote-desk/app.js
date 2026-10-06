// Three throwaway UI directions, selected by ?variant=A|B|C. All state is in memory.
(() => {
  'use strict';
  const $ = id => document.getElementById(id);
  const esc = value => String(value).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const paths = {
    logo: '<path d="M3 15V4h12M21 9v11H9M8 15l8-8M11 7h5v5"/>',
    monitor: '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M12 17v4"/>',
    phone: '<rect x="7" y="2" width="10" height="20" rx="2"/><path d="M11 18h2"/>',
    tablet: '<rect x="4" y="2" width="16" height="20" rx="2"/><path d="M11 18h2"/>',
    grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
    activity: '<path d="M3 12h4l3-8 4 16 3-8h4"/>',
    settings: '<path d="M12 3l2 3 4-.2.7 3.8 2.3 2.4-2.3 2.4-.7 3.8-4-.2-2 3-2-3-4 .2-.7-3.8L3 12l2.3-2.4L6 5.8l4 .2z"/><circle cx="12" cy="12" r="3"/>',
    shield: '<path d="M12 3l8 3v5c0 5-4 8-8 10-4-2-8-5-8-10V6z"/><path d="M8.5 11.5l2.5 2.5 4.5-5"/>',
    lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 018 0v3M12 14v3"/>',
    link: '<path d="M10 13a4 4 0 006 0l3-3a4 4 0 00-6-6l-2 2M14 11a4 4 0 00-6 0l-3 3a4 4 0 006 6l2-2"/>',
    arrow: '<path d="M4 12h16m-6-6 6 6-6 6"/>', right: '<path d="M9 5l7 7-7 7"/>', chevron: '<path d="M9 5l7 7-7 7"/>',
    left: '<path d="M15 5l-7 7 7 7"/>', plus: '<path d="M12 5v14M5 12h14"/>', close: '<path d="M6 6l12 12M6 18L18 6"/>',
    search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="M16 16l5 5"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>', check: '<path d="M5 12l4 4L19 6"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>',
    rotate: '<path d="M4 8a8 8 0 0115-1M20 3v5h-5M20 16a8 8 0 01-15 1M4 21v-5h5"/>',
    star: '<path d="M12 3l2.8 5.6 6.2.9-4.5 4.4 1.1 6.1-5.6-2.9L6.4 20l1.1-6.1L3 9.5l6.2-.9z"/>',
    mouse: '<rect x="6" y="2" width="12" height="20" rx="6"/><path d="M12 2v7M6 10h12"/>',
    keyboard: '<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M7 16h10"/>',
    expand: '<path d="M8 3H3v5M16 3h5v5M21 16v5h-5M8 21H3v-5"/>',
    folder: '<path d="M3 6h7l2 3h9v11H3z"/>', file: '<path d="M5 3h9l5 5v13H5zM14 3v6h5M9 13h6M9 17h6"/>',
    copy: '<rect x="8" y="8" width="12" height="13" rx="2"/><path d="M15 8V3H3v13h5"/>',
    user: '<circle cx="12" cy="7" r="4"/><path d="M4 21v-2a8 8 0 0116 0v2"/>',
    key: '<circle cx="8" cy="8" r="5"/><path d="M11.5 11.5L21 21M16 16l3-3M18 18l3-3"/>',
    power: '<path d="M12 2v10M6 5a9 9 0 1012 0"/>',
    cursor: '<path d="M5 3l14 9-7 1-3 7z"/>',
    more: '<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>',
    wifi: '<path d="M3 8a15 15 0 0118 0M6 12a10 10 0 0112 0M9 16a5 5 0 016 0M12 20h.01"/>',
    hand: '<path d="M8 13V7a2 2 0 014 0v5-7a2 2 0 014 0v7-5a2 2 0 014 0v8c0 5-3 7-7 7-3 0-4-2-6-4l-3-4c-1-2 1-3 3-2l3 3"/>',
    'chevron-up': '<path d="M6 15l6-6 6 6"/>',
    'chevron-down': '<path d="M6 9l6 6 6-6"/>',
    eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    upload: '<path d="M12 16V3m-5 5 5-5 5 5M4 15v6h16v-6"/>',
    download: '<path d="M12 3v13m-5-5 5 5 5-5M4 15v6h16v-6"/>',
    bolt: '<path d="M13 2L4 14h7l-1 8 10-13h-7z"/>',
  };
  const icon = (name, size = 20) => `<svg class="icon" width="${size}" height="${size}" style="width:${size}px;height:${size}px" viewBox="0 0 24 24" aria-hidden="true">${paths[name] || paths.info}</svg>`;
  const query = new URLSearchParams(location.search);
  const pick = (key, options, fallback) => options.includes(query.get(key)) ? query.get(key) : fallback;
  const state = {
    variant: pick('variant', ['A', 'B', 'C'], 'A'), role: pick('role', ['controller', 'host'], 'controller'),
    viewport: pick('viewport', ['pc', 'tablet', 'phone'], 'pc'), landscape: query.has('landscape') ? query.get('landscape') === 'true' : query.get('viewport') !== 'phone',
    page: 'devices', session: 'idle', hostPhase: 'off', connectMode: 'id', deviceId: '842 916 305', ip: '192.168.1.12', port: '21118',
    selected: 'studio', filter: 'all', search: '', inputMode: 'trackpad', quality: 'balanced', keyboard: false, tools: false,
    controlRequested: false, controlGranted: false, formError: '', event: '', records: [],
    accessMode: 'unattended', defaultAccessMode: 'unattended', currentAccessMode: null, connectionPolicy: 'direct-first', activeAccess: null, defaultRequestedMode: 'control', connectRequestedMode: null,
    unattended: { enabled: true, trusted: true, password: 'Desk2026!', control: true, upload: false, download: false },
    filePermissions: { upload: false, download: false }, fileTransfers: [], fileSelections: { local: [], remote: [] },
    filesOpen: false, trackpad: false, toolbarCollapsed: false, screenFit: 'fit',
    remoteEditor: false, remoteText: '',
    devices: [
      { id: 'studio', name: '工作室电脑', deviceId: '842 916 305', os: 'Windows 11', online: true, color: '', subtitle: '今天 14:32 连接', tag: '常用设备' },
      { id: 'study', name: '书房电脑', deviceId: '682 193 074', os: 'Windows 11', online: true, color: 'blue', subtitle: '昨天连接', tag: '个人设备' },
      { id: 'office', name: '办公室电脑', deviceId: '317 502 846', os: 'Windows 10', online: false, color: 'gray', subtitle: '3 天前在线', tag: '个人设备' },
    ],
  };
  // Each target owns its own unattended profile; the first sample is shared with the host view.
  state.devices[0].unattended = state.unattended;
  state.devices[1].unattended = { enabled: true, trusted: true, password: 'Study2026!', control: false, upload: false, download: false };
  state.devices[2].unattended = { enabled: false, trusted: false, password: '', control: false, upload: false, download: false };
  const emptyDeviceAccess = Object.freeze({ enabled: false, trusted: false, password: '', control: false, upload: false, download: false });
  const deviceAccess = device => device?.unattended || emptyDeviceAccess;
  const variants = { A: ['设备工作台', '设备与连接并重，适合日常使用'], B: ['快速连接', '一个输入焦点，适合临时协助'], C: ['会话工作区', '设备与详情并列，适合固定设备'] };
  const sessionNames = { idle: '未连接', pending: '等待批准', view: '仅查看', control: '可操作', ended: '已结束', rejected: '已拒绝', timeout: '批准超时', identity: '身份变化', network: '网络中断' };
  const hostNames = { off: '未启用', inviting: '邀请中', pending: '待批准', view: '共享画面', control: '允许操作', ended: '已结束' };
  let toastTimer;
  let remoteKeyboard;
  const notify = message => { $('toast').textContent = message; $('toast').classList.add('toast-visible'); clearTimeout(toastTimer); toastTimer = setTimeout(() => $('toast').classList.remove('toast-visible'), 3000); };
  const selectedDevice = () => state.devices.find(d => d.id === state.selected) || state.devices[0];
  const sessionDevice = () => state.activeDevice || selectedDevice();
  const effectiveConnectMode = () => state.connectMode === 'ip' ? 'ip' : state.connectionPolicy === 'relay-only' ? 'relay' : state.connectMode;
  const route = () => ({ id: '协调直连', ip: 'IP 直连', relay: '加密中继' }[state.activeMode || effectiveConnectMode()]);
  const actionButton = (action, label, kind = '', glyph = '') => `<button class="btn ${kind}" data-action="${action}">${glyph ? icon(glyph, 16) : ''}${label}</button>`;
  const record = text => state.records.unshift({ text, time: new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }), name: sessionDevice().name, path: route() });

  function updateURL() {
    const params = new URLSearchParams({ variant: state.variant, role: state.role, viewport: state.viewport });
    params.set('landscape', String(state.landscape));
    history.replaceState(null, '', `${location.pathname}?${params}`);
  }

  function renderLab() {
    $('lab').innerHTML = `<div class="lab-brand">${icon('logo', 22)}REMOTE DESK <span>DESIGN LAB</span></div>
      <div class="lab-controls"><div class="segmented" aria-label="端角色">${[['controller', '控制端'], ['host', '被控端']].map(([key, label]) => `<button data-action="role-${key}" class="${state.role === key ? 'active' : ''}" aria-pressed="${state.role === key}">${label}</button>`).join('')}</div>
      <div class="segmented" aria-label="预览尺寸">${[['phone', '手机', 'phone'], ['tablet', '平板', 'tablet'], ['pc', 'PC', 'monitor']].map(([key, label, glyph]) => `<button data-action="viewport-${key}" class="${state.viewport === key ? 'active' : ''}" aria-pressed="${state.viewport === key}">${icon(glyph, 15)}${label}</button>`).join('')}</div>
      ${state.viewport !== 'pc' ? `<button class="lab-rotate" aria-label="切换横竖屏" title="切换横竖屏" data-action="rotate">${icon('rotate', 16)}</button>` : ''}</div>
      <div class="row"><span class="lab-note">交互原型 · 所有连接均为模拟</span><button class="lab-detail row" data-action="design">${icon('info', 16)}设计说明</button></div>`;
    $('switcher').innerHTML = state.role === 'controller' ? `<div class="proto-switch" aria-label="设计方案切换"><button class="icon-btn" data-action="variant-prev" aria-label="上一个设计方案">${icon('left', 15)}</button><div class="variant-label"><small>LAYOUT EXPLORATION · ${state.variant === 'A' ? '推荐方案' : '对比方案'}</small><strong>${state.variant} · ${variants[state.variant][0]}</strong></div><button class="icon-btn" data-action="variant-next" aria-label="下一个设计方案">${icon('right', 15)}</button><div class="variant-dots">${['A', 'B', 'C'].map(v => `<button data-action="variant-${v}" aria-label="方案 ${v}：${variants[v][0]}" aria-pressed="${state.variant === v}" class="${state.variant === v ? 'active' : ''}">${v}</button>`).join('')}</div></div>` : `<div class="proto-switch"><div class="variant-label"><small>WINDOWS HOST · 远程访问</small><strong>临时协助 / 无人值守 / 独立文件授权</strong></div><button class="icon-btn" data-action="role-controller" title="查看控制端" aria-label="查看控制端">${icon('arrow', 17)}</button></div>`;
  }

  function navigation(mobile = false) {
    return [['devices', 'grid', '设备'], ['activity', 'clock', '最近连接'], ['settings', 'settings', '偏好设置']].map(([key, glyph, label]) => `<button class="${mobile ? '' : 'nav-item'} ${state.page === key ? 'active' : ''}" data-action="page-${key}" aria-label="${label}" ${state.page === key ? 'aria-current="page"' : ''}>${icon(glyph, 19)}<span>${label}</span>${!mobile && key === 'devices' ? `<span class="nav-count">${state.devices.length.toString().padStart(2, '0')}</span>` : ''}</button>`).join('');
  }
  function shell(content) {
    return `<div class="app-shell"><aside class="app-sidebar"><div class="brand"><span class="brand-mark">${icon('logo', 31)}</span><div class="brand-name">Remote Desk<small>STAY CONNECTED</small></div></div><div><div class="sidebar-label">WORKSPACE</div><nav class="app-nav" aria-label="控制端导航">${navigation()}</nav></div><div class="sidebar-bottom"><div class="sidebar-tip">${icon('shield', 22)}<strong>连接由你掌控</strong>临时协助需现场确认；自己的设备可按预设权限连接。</div><div class="local-profile"><div class="avatar">RD</div><div>本地工作区<small>个人设备 · 自建服务</small></div></div></div></aside><main class="app-main"><div class="topline"><div class="crumb">工作空间 ${icon('right', 11)} <b>${{ devices: '我的设备', activity: '最近连接', settings: '偏好设置' }[state.page]}</b></div><div class="row"><span class="date">9 月 30 日，星期三</span><span class="badge green"><i class="dot"></i>服务可用</span></div></div>${['pending', 'view', 'control'].includes(state.session) ? `<div class="resume-banner"><span><i class="dot"></i>${esc(sessionDevice().name)} · ${sessionNames[state.session]}</span><button data-action="resume-session">返回会话 ${icon('arrow', 13)}</button></div>` : ''}${content}</main><nav class="mobile-nav" aria-label="移动端导航">${navigation(true)}</nav></div>`;
  }
  function heading(title = '我的设备', subtitle = '让距离，留在屏幕之外。') {
    return `<div class="heading"><div><div class="eyebrow">YOUR WORKSPACE, ANYWHERE</div><h1>${title}</h1><p>${subtitle}</p></div>${actionButton('add-device', '添加设备', '', 'plus')}</div>`;
  }
  function passwordFields(id = 'access-password') {
    return `<label class="field-label" for="${id}">访问密码</label><div class="password-field"><input id="${id}" name="password" type="password" aria-label="访问密码" placeholder="输入被控端设置的访问密码" autocomplete="off"><button class="password-eye" type="button" data-action="toggle-password" aria-label="显示或隐藏访问密码">${icon('eye', 17)}</button></div><div class="demo-credential"><span>原型演示 · 请勿输入真实密码</span><button type="button" data-action="demo-password">填入示例密码</button></div><div class="connect-policy"><span>进入桌面后</span><select id="requested-mode" aria-label="连接后的操作模式"><option value="control" ${(state.connectRequestedMode || state.defaultRequestedMode) === 'control' ? 'selected' : ''}>控制模式</option><option value="view" ${(state.connectRequestedMode || state.defaultRequestedMode) === 'view' ? 'selected' : ''}>仅查看</option></select><span>按被控端预设权限</span></div>`;
  }
  function connectCard() {
    const unattended = (state.currentAccessMode || state.defaultAccessMode) === 'unattended';
    return `<section class="connect-card"><div class="connect-title">${icon('link')}连接远程设备</div><div class="connect-tabs" role="tablist" aria-label="连接方式">${[['id', '设备 ID'], ['ip', 'IP 直连'], ['relay', '中继']].map(([mode, label]) => `<button role="tab" aria-selected="${state.connectMode === mode}" data-action="mode-${mode}" class="${state.connectMode === mode ? 'active' : ''}">${label}</button>`).join('')}</div><form id="connect-form"><label class="field-label" for="${state.connectMode === 'ip' ? 'ip-input' : 'device-id-input'}">${state.connectMode === 'ip' ? 'IP 地址与端口' : '远程设备 ID'}</label><div class="connect-fields">${state.connectMode === 'ip' ? `<input id="ip-input" name="ip" aria-label="IP 地址" value="${esc(state.ip)}" placeholder="192.168.1.12 / IPv6" autocomplete="off"><input class="port-field" id="port-input" name="port" aria-label="端口" value="${esc(state.port)}" inputmode="numeric" placeholder="21118">` : `<input id="device-id-input" name="deviceId" aria-label="远程设备 ID" value="${esc(state.deviceId)}" inputmode="numeric" placeholder="输入对方的设备 ID" autocomplete="off">`}</div><div class="connect-access" aria-label="访问方式"><button type="button" data-action="access-unattended" class="${unattended ? 'active' : ''}" aria-pressed="${unattended}">无人值守 · 密码</button><button type="button" data-action="access-attended" class="${unattended ? '' : 'active'}" aria-pressed="${!unattended}">临时协助</button></div>${unattended ? passwordFields() : '<p class="input-hint" style="font-size:11px;color:#819477;margin:10px 0">使用单次协助码，并由对方现场批准。</p>'}<p class="error-inline" id="connect-error" role="alert">${esc(state.formError)}</p><button type="submit" class="btn primary block">${unattended ? '连接桌面' : '请求协助'} ${icon('arrow', 14)}</button><div class="connect-foot"><span>${icon('lock', 12)}${unattended ? '密码验证后进入桌面' : '连接后仍需对方批准'}</span><span>${state.connectMode === 'ip' ? '仅尝试指定 IP' : state.connectionPolicy === 'relay-only' ? '策略：始终使用中继' : state.connectMode === 'relay' ? '使用自建中继' : '策略：直连优先'}</span></div></form></section>`;
  }
  function welcome() {
    return `<section class="welcome"><div class="welcome-content"><div class="eyebrow">A LITTLE CLOSER, EVERY DAY</div><h2>下一站，<br>你的桌面。</h2><p>熟悉的工作空间，<br>就在手边。</p><div class="welcome-tag">${icon('shield', 12)}安心连接 · 随时协作</div></div><div class="desk-art" aria-hidden="true"><div class="art-disc"></div><div class="art-spark">✦</div><div class="art-screen"><div class="art-window">${icon('logo', 32)}</div></div><div class="art-phone">${icon('check', 20)}</div></div></section>`;
  }
  const visibleDevices = () => state.devices.filter(d => (state.filter !== 'online' || d.online) && (state.filter !== 'favorites' || d.id === 'studio') && `${d.name} ${d.deviceId}`.toLowerCase().includes(state.search.toLowerCase()));
  function deviceCard(d) {
    return `<article class="device-card"><div class="device-card-top"><div class="device-icon ${d.color}">${icon('monitor', 23)}</div><span class="status"><i class="dot ${d.online ? '' : 'offline'}"></i>${d.online ? '在线' : '离线'}</span></div><h3>${esc(d.name)}</h3><div class="device-sub">${icon('grid', 10)}${d.os}<span>·</span>${esc(d.deviceId)}</div><div class="device-footer"><span>${d.subtitle}</span><button data-action="connect-${d.id}" ${d.online ? '' : 'disabled'} aria-label="${d.online ? '连接' : '离线不可连接'}${esc(d.name)}">${d.online ? '连接' : '不可连接'}${icon('arrow', 13)}</button></div></article>`;
  }
  function deviceSection() {
    return `<section><div class="device-section-head"><div class="device-section-title"><h2>我的设备</h2><span class="count">${state.devices.length}</span></div><div class="list-tools"><label class="search-field">${icon('search', 14)}<input id="search" aria-label="搜索设备" placeholder="搜索设备" value="${esc(state.search)}"></label><button class="icon-btn" title="查看会话工作区" aria-label="查看会话工作区" data-action="variant-C">${icon('grid', 16)}</button></div></div><div class="filter-tabs">${[['all', '全部设备'], ['online', '在线'], ['favorites', '常用']].map(([f, label]) => `<button data-action="filter-${f}" class="${state.filter === f ? 'active' : ''}" aria-pressed="${state.filter === f}">${label}</button>`).join('')}</div><div class="device-grid" id="device-grid">${visibleDevices().map(deviceCard).join('') || '<p class="empty">没有找到匹配的设备</p>'}</div></section>`;
  }
  function supportLine() { return `<div class="support-line"><span>${icon('shield', 13)}临时协助或密码连接，权限由被控端授予</span><button data-action="preview-session">预览远程会话 ${icon('arrow', 12)}</button></div>`; }
  function VariantA() { return shell(`${heading()}<div class="overview">${welcome()}${connectCard()}</div>${deviceSection()}${supportLine()}`); }
  function VariantB() {
    return shell(`<div class="quick-main"><div class="quick-intro"><div class="quick-symbol">${icon('link', 27)}</div><div class="eyebrow">ONE CONNECTION AWAY</div><h1>现在，连接哪台电脑？</h1><p>输入设备 ID，开始一次安心的远程协助。</p></div>${connectCard()}<div class="device-section-head"><div class="device-section-title"><h2>最近的设备</h2><span class="count">${state.devices.length}</span></div><button class="text-btn" data-action="add-device">${icon('plus', 13)}添加</button></div><div class="recent-list">${state.devices.map(d => `<div class="recent-device"><div class="device-icon ${d.color}">${icon('monitor', 20)}</div><div class="recent-info"><h3>${esc(d.name)}</h3><p><i class="dot ${d.online ? '' : 'offline'}"></i> ${d.os} · ${d.subtitle}</p></div><button class="btn ${d.online ? 'soft' : ''}" data-action="connect-${d.id}" ${d.online ? '' : 'disabled'}>${d.online ? '连接' : '离线'}${d.online ? icon('arrow', 12) : ''}</button></div>`).join('')}</div>${supportLine()}</div>`);
  }
  function VariantC() {
    const d = selectedDevice();
    return shell(`${heading('连接工作区', '选一台设备，让工作顺手继续。')}<div class="workbench"><aside class="device-rail"><h3>设备列表 <span class="muted">/ ${state.devices.length}</span></h3>${state.devices.map(item => `<button class="rail-device ${item.id === d.id ? 'active' : ''}" data-action="select-${item.id}" aria-pressed="${item.id === d.id}"><div class="device-icon ${item.color}">${icon('monitor', 20)}</div><span><strong>${esc(item.name)}</strong><small><i class="dot ${item.online ? '' : 'offline'}"></i> ${item.online ? '可连接' : '离线'}</small></span></button>`).join('')}<button class="text-btn" data-action="add-device">${icon('plus', 14)}添加设备</button></aside><section class="workbench-detail"><div class="row between"><div><div class="eyebrow">${d.tag === '常用设备' ? 'FAVORITE DEVICE' : 'PERSONAL DEVICE'}</div><h2 style="margin-top:10px">${esc(d.name)}</h2></div>${icon('monitor', 28)}</div><div class="detail-device-illustration">${icon('monitor', 88)}<span class="badge ${d.online ? 'green' : ''}"><i class="dot ${d.online ? '' : 'offline'}"></i>${d.online ? '设备在线' : '设备离线'}</span></div><dl class="detail-data"><div><dt>设备 ID</dt><dd>${esc(d.deviceId)}</dd></div><div><dt>系统</dt><dd>${d.os}</dd></div><div><dt>连接方式</dt><dd>${state.connectionPolicy === 'relay-only' ? '始终使用中继' : '直连优先'} · ${(state.currentAccessMode || state.defaultAccessMode) === 'unattended' ? '密码连接' : '临时协助'}</dd></div><div><dt>访问方式</dt><dd>${(state.currentAccessMode || state.defaultAccessMode) === 'unattended' ? '访问密码 · 预设权限' : '现场批准 · 仅查看开始'}</dd></div></dl><div class="detail-bottom"><button class="btn primary" data-action="connect-${d.id}" ${d.online ? '' : 'disabled'}>${icon('link', 16)}${d.online ? '连接设备' : '设备离线'}</button>${actionButton('custom-connect', '其他方式')}</div></section></div>${supportLine()}`);
  }

  function remoteDesktop() {
    const note = `<div class="remote-editor-toolbar"><span>文件　编辑　查看</span><span>输入演示</span></div><pre id="remote-note-content" class="remote-note-content ${state.remoteText ? '' : 'is-empty'}">${esc(state.remoteText || '点击工具栏的键盘，直接在这里输入。')}</pre>`;
    return `<div class="remote-display ${state.session === 'control' && state.inputMode === 'touch' ? 'interactive' : ''}" aria-label="模拟远程桌面"><div class="remote-wallpaper"><div class="wallpaper-fold"></div><div class="wallpaper-fold second"></div></div><div class="desktop-icons"><div>${icon('monitor', 24)}此电脑</div><div>${icon('folder', 24)}工作空间</div></div><div class="remote-window ${state.remoteEditor ? 'note-window' : ''}"><div class="window-top"><span>${icon(state.remoteEditor ? 'file' : 'folder', 11)} ${state.remoteEditor ? '记事本 · 输入演示' : '工作空间'}</span><span class="window-controls">−　□　×</span></div>${state.remoteEditor ? note : `<div class="window-toolbar"><span>←　→　↑</span><span>此电脑　›　文档　›　工作空间</span></div><div class="window-content"><div class="window-side"><span>快速访问</span><span>桌面</span><span class="selected">文档</span><span>下载</span><span>图片</span><span>此电脑</span></div><div class="window-files">${['项目资料', '设计方案', '会议记录', '参考素材', '共享文档', '本周计划'].map(label => `<div><div class="folder"></div>${label}</div>`).join('')}</div></div>`}</div><div class="remote-cursor" id="remote-cursor">${icon('cursor', 21)}</div><span class="mock-label">DESKTOP PREVIEW · 模拟画面</span><div class="remote-taskbar"><div class="windows-mark"><i></i><i></i><i></i><i></i></div>${icon('search', 13)}${icon('folder', 13)}${icon('monitor', 13)}<div class="tray">14:32<br>2026/09/30</div></div></div>`;
  }
  function SessionView() { return window.SessionPrototype.render(state, helpers); }
  function PendingView() {
    return `<div class="waiting"><section class="waiting-card"><div class="wait-symbol">${icon('clock', 31)}</div><div class="eyebrow">WAITING FOR CONFIRMATION</div><h1 style="margin-top:12px">等对方点一下，马上连接。</h1><p>已向「${esc(sessionDevice().name)}」发出查看请求。<br>等待现场批准，最长 60 秒。</p><div class="progress-steps"><div class="progress-step"><span class="step-circle">${icon('check', 13)}</span>网络已连接 · ${route()}</div><div class="progress-step"><span class="step-circle">${icon('check', 13)}</span>被控设备身份与加密通道已核验</div><div class="progress-step"><span class="step-circle">${icon('clock', 13)}</span>等待本机批准 · 此时无法查看屏幕</div></div><div class="badge">双方核对配对码 <b style="letter-spacing:2px;margin-left:8px">492 816</b></div><div class="waiting-actions">${actionButton('disconnect', '取消请求')}${actionButton('role-host', '去被控端批准', 'soft', 'arrow')}</div><div class="simulation-bar"><label>原型演示 · 模拟批准结果</label><button data-action="simulate-approve">批准查看</button><button data-action="simulate-reject">拒绝</button><button data-action="simulate-timeout">超时</button><button data-action="simulate-identity">身份变化</button></div></section></div>`;
  }
  function EndView() {
    const errors = { rejected: ['对方拒绝了本次连接', '屏幕没有共享，操作权限没有开放。请与对方沟通后重新发起协助。'], timeout: ['等待批准已超时', '60 秒内没有获得批准。本次请求已结束，重新连接需要再次确认。'], identity: ['设备身份发生变化', '已停止连接。请通过可信方式重新核对设备身份，然后再发起连接。'], network: ['连接已中断', '画面和操作已停止。重新连接需要重新批准，不沿用上一次的权限。'], ended: ['本次连接已结束', '屏幕共享和操作权限已关闭。让工作稍停一下，下次再继续。'] };
    const [title, text] = errors[state.session] || errors.ended;
    return `<div class="waiting wait-error"><section class="waiting-card"><div class="wait-symbol">${icon(state.session === 'ended' ? 'check' : 'shield', 31)}</div><h1>${title}</h1><p>${text}</p>${actionButton('page-devices', '返回我的设备', 'primary', 'left')}<div style="margin-top:24px" class="badge">${icon('lock', 12)}当前没有有效授权</div></section></div>`;
  }
  function ActivityView() {
    return shell(`<div class="heading"><div><div class="eyebrow">RECENT CONNECTIONS</div><h1>最近连接</h1><p>每次连接与授权，都有迹可循。</p></div></div>${state.records.length ? `<table class="history-table"><thead><tr><th>设备</th><th>事件</th><th>路径</th><th>时间</th></tr></thead><tbody>${state.records.map(r => `<tr><td>${esc(r.name)}</td><td>${esc(r.text)}</td><td>${r.path}</td><td>${r.time}</td></tr>`).join('')}</tbody></table>` : '<div class="empty">还没有本次演示的连接记录。连接一台设备后，会在这里显示。</div>'}<p class="settings-note">记录连接与权限变化，不保存远程画面或输入内容。</p>`);
  }
  function SettingsView() {
    return shell(`<div class="heading"><div><div class="eyebrow">MAKE YOURSELF AT HOME</div><h1>偏好设置</h1><p>这里的默认值会用于下一次连接；当前会话权限由被控端单独决定。</p></div></div><div class="settings-grid"><section class="panel setting-card"><h3>连接服务</h3><div class="setting-row"><span>服务来源</span><b>自建服务 · 示例配置</b></div><div class="setting-row"><span>连接策略</span><select id="setting-policy" aria-label="连接策略"><option value="direct-first" ${state.connectionPolicy === 'direct-first' ? 'selected' : ''}>直连优先，安全中继备用</option><option value="relay-only" ${state.connectionPolicy === 'relay-only' ? 'selected' : ''}>始终使用加密中继</option></select></div><div class="setting-row"><span>默认访问方式</span><select id="setting-access" aria-label="默认访问方式"><option value="unattended" ${state.defaultAccessMode === 'unattended' ? 'selected' : ''}>无人值守 · ID + 密码</option><option value="attended" ${state.defaultAccessMode === 'attended' ? 'selected' : ''}>临时协助 · 现场批准</option></select></div><div class="setting-row"><span>默认进入模式</span><select id="setting-mode" aria-label="默认进入模式"><option value="control" ${(state.connectRequestedMode || state.defaultRequestedMode) === 'control' ? 'selected' : ''}>控制模式</option><option value="view" ${(state.connectRequestedMode || state.defaultRequestedMode) === 'view' ? 'selected' : ''}>仅查看</option></select></div><p class="settings-note">IP 直连始终遵循用户填写的地址；默认策略不会把明确选择的 IP 改成中继。</p></section><section class="panel setting-card"><h3>远程桌面</h3><div class="setting-row"><span>默认输入方式</span><div class="mode-toggle"><button data-action="input-trackpad" class="${state.inputMode === 'trackpad' ? 'active' : ''}">触控板</button><button data-action="input-touch" class="${state.inputMode === 'touch' ? 'active' : ''}">直接触控</button></div></div><div class="setting-row"><span>画面显示</span><select id="setting-fit" aria-label="画面显示"><option value="fit" ${state.screenFit === 'fit' ? 'selected' : ''}>适应窗口</option><option value="fill" ${state.screenFit === 'fill' ? 'selected' : ''}>填满画面</option></select></div><div class="setting-row"><span>画面质量</span><select id="setting-quality" aria-label="画面质量"><option value="balanced" ${state.quality === 'balanced' ? 'selected' : ''}>均衡 · 自动适应</option><option value="smooth" ${state.quality === 'smooth' ? 'selected' : ''}>流畅 · 节省流量</option><option value="clear" ${state.quality === 'clear' ? 'selected' : ''}>清晰 · 细节优先</option></select></div><p class="settings-note">输入、显示和画质偏好会立即反映到当前模拟会话，并作为下一次默认值；实际媒体与系统输入能力仍需验证。</p></section></div>`);
  }

  function render() {
    updateURL(); renderLab();
    let content;
    const captionAccess = deviceAccess(['pending', 'view', 'control'].includes(state.session) ? sessionDevice() : selectedDevice());
    if (state.role === 'host') content = window.HostPrototype.render(state, helpers);
    else if (state.page === 'session') content = state.session === 'pending' ? PendingView() : ['view', 'control'].includes(state.session) ? SessionView() : EndView();
    else if (state.page === 'activity') content = ActivityView();
    else if (state.page === 'settings') content = SettingsView();
    else content = ({ A: VariantA, B: VariantB, C: VariantC }[state.variant])();
    $('stage').innerHTML = `<div class="stage-heading"><strong>${state.role === 'host' ? '02 / 被控端 · Windows' : '01 / 控制端 · ' + variants[state.variant][1]}</strong><span class="stage-spec">${state.viewport === 'phone' ? (state.landscape ? 'PHONE · 横屏' : 'PHONE · 390 px 竖屏') : state.viewport === 'tablet' ? 'TABLET · 自适应双栏' : 'DESKTOP · 完整工作台'}　/　2026.09</span></div><div class="surface" data-viewport="${state.viewport}" data-landscape="${state.landscape}"><div class="device-status"><span>9:41</span><span>●●● ${icon('wifi', 12)} ▰</span></div>${content}</div><div class="caption"><span>方案 ${state.variant} · ${variants[state.variant][0]}</span><span>控制端：${sessionNames[state.session] || state.session}</span><span>被控端：${['off', 'ended'].includes(state.hostPhase) && captionAccess.enabled && captionAccess.trusted ? '无人值守待连接' : captionAccess.enabled ? '无人值守待登记' : hostNames[state.hostPhase]}</span><span>权限：${state.controlGranted ? '查看 + 操作' : state.session === 'view' ? '仅查看' : '无'}</span><span>文件：${state.filePermissions.upload ? '可上传' : '上传关闭'} / ${state.filePermissions.download ? '可下载' : '下载关闭'}</span></div>`;
    remoteKeyboard?.sync();
    bindPointer();
  }
  const helpers = { icon, escape: esc, notify, render, sessionDevice, route, desktop: remoteDesktop, resetFileWorkspace, keyboardActive: () => remoteKeyboard?.isActive() || false,
    stopTransfers: (reason, direction) => window.FilePrototype.stop(state, reason, direction),
    clearSession: (reason = 'ended') => finishSession(reason),
  };

  function updateRemoteNote() {
    const note = $('remote-note-content');
    if (note) { note.textContent = state.remoteText || '点击工具栏的键盘，直接在这里输入。'; note.classList.toggle('is-empty', !state.remoteText); note.scrollTop = note.scrollHeight; }
  }
  function receiveRemoteEdit(remove, text) {
    if (state.session !== 'control' || !state.controlGranted) return;
    const value = Array.from(state.remoteText);
    state.remoteText = value.slice(0, Math.max(0, value.length - remove)).join('') + text;
    updateRemoteNote();
  }
  remoteKeyboard = window.RemoteKeyboardPrototype.create({
    allowed: () => state.keyboard && state.role === 'controller' && state.page === 'session' && state.session === 'control' && state.controlGranted && !state.filesOpen && !state.tools && !state.trackpad && !$('overlay').children.length,
    edit: receiveRemoteEdit,
    key: name => notify(`模拟远端按键：${name}`),
    status: active => { const label = $('remote-input-status'); if (label) label.textContent = active ? '直接输入，中文选词后自动提交' : '输入已暂停，点击键盘继续'; },
  });

  function dialog(body, wide = false) {
    remoteKeyboard.stop();
    $('overlay').innerHTML = `<div class="overlay-backdrop"><section class="dialog ${wide ? 'design-dialog' : ''}" role="dialog" aria-modal="true" aria-label="${wide ? '设计说明' : '连接与设备'}"><div class="dialog-head"><span class="eyebrow">REMOTE DESK</span><button class="icon-btn" data-action="close-dialog" aria-label="关闭对话框">${icon('close', 18)}</button></div>${body}</section></div>`;
    $('overlay').querySelector('input,button')?.focus();
  }
  function closeDialog() { $('overlay').innerHTML = ''; }
  function openAttendedCredentials() {
    if (state.hostPhase !== 'inviting') {
      dialog(`<h2>对方还没有开启协助</h2><p>请让对方在「${esc(selectedDevice().name)}」上开启临时协助，再获取本次协助码。</p>${actionButton('open-host', '去被控端开启协助', 'primary block', 'arrow')}<div class="simulation-bar" style="margin-top:20px"><label>原型演示</label><button data-action="simulate-invite">模拟对方开启临时协助</button></div>`);
      return;
    }
    dialog(`<h2>输入临时协助码</h2><p>正在连接「${esc(selectedDevice().name)}」。向对方获取本次临时协助码，连接后仍需现场批准。</p><form id="credential-form"><label for="assist-code">临时协助码</label><input id="assist-code" name="code" value="728 461" inputmode="numeric" autocomplete="off"><div class="input-hint">原型示例：728 461 · 仅本次有效</div><p class="error-inline" id="credential-error" role="alert"></p><button class="btn primary block" type="submit">请求查看 ${icon('arrow', 15)}</button></form>`);
  }
  function openCredentials() {
    if ((state.currentAccessMode || state.defaultAccessMode) === 'attended') { openAttendedCredentials(); return; }
    dialog(`<h2>连接「${esc(selectedDevice().name)}」</h2><p>输入这台设备的访问密码，验证后直接进入桌面。设备 ID：${esc(selectedDevice().deviceId)}</p><form id="password-form">${passwordFields('dialog-password')}<p class="error-inline" id="password-error" role="alert"></p><button type="submit" class="btn primary block">连接桌面 ${icon('arrow', 15)}</button></form><button class="text-btn" data-action="use-attended">改用临时协助</button>`);
  }
  function authenticate(password, errorId = 'connect-error') {
    const access = deviceAccess(selectedDevice());
    const failure = !access.enabled ? '这台设备未启用无人值守，请开启后再连接，或改用临时协助。'
      : !access.trusted ? '当前控制设备尚未登记，请在被控端完成登记后再连接。'
      : password !== access.password ? '访问密码不正确，请重新输入。' : '';
    if (failure) { const error = $(errorId); if (error) error.textContent = failure; return; }
    resetFileWorkspace();
    state.remoteText = ''; state.remoteEditor = false;
    state.activeDevice = { ...selectedDevice() }; state.activeMode = effectiveConnectMode(); state.activeAccess = 'unattended';
    state.controlGranted = access.control;
    state.session = state.controlGranted && (state.connectRequestedMode || state.defaultRequestedMode) === 'control' ? 'control' : 'view';
    state.hostPhase = state.controlGranted ? 'control' : 'view';
    state.filePermissions = { upload: access.upload, download: access.download };
    state.fileRequests = { upload: false, download: false };
    state.controlRequested = false; state.keyboard = false; state.tools = false; state.trackpad = false; state.filesOpen = false; state.toolbarCollapsed = false; state.event = ''; state.formError = '';
    state.hostTab = 'home'; state.hostAccessTab = 'unattended'; state.connectRequestedMode = null; state.page = 'session'; closeDialog(); record('访问密码验证通过 · 无人值守'); render();
  }
  function finishSession(result) {
    remoteKeyboard.stop(); state.remoteText = ''; state.remoteEditor = false;
    window.FilePrototype.stop(state, '连接已结束，传输已停止');
    state.session = result; state.hostPhase = 'ended'; state.hostEndReason = '本次连接已结束，全部会话授权已撤销。'; state.page = 'session';
    state.keyboard = false; state.tools = false; state.trackpad = false; state.filesOpen = false; state.toolbarCollapsed = false;
    state.controlGranted = false; state.controlRequested = false; state.currentAccessMode = null; state.connectRequestedMode = null; state.filePermissions = { upload: false, download: false }; state.fileRequests = { upload: false, download: false }; state.event = '';
    record(sessionNames[result] || '已结束'); render();
  }
  function resetFileWorkspace() {
    state.fileTransfers = []; state.fileSelections = { local: [], remote: [] };
    state.fileRequests = { upload: false, download: false }; state.fileFolder = { local: 'documents', remote: 'documents' };
    state.fileTab = 'local'; delete state.fileEntries;
  }
  function startConnect(id, password) {
    if (['pending', 'view', 'control'].includes(state.session)) { notify('当前已有一个会话，请先结束本次连接'); state.page = 'session'; render(); return; }
    if (id) { state.selected = id; state.deviceId = selectedDevice().deviceId; }
    if (!selectedDevice().online) { notify('设备已离线，请确认远端电脑在线'); return; }
    if ((state.currentAccessMode || state.defaultAccessMode) === 'unattended' && password !== undefined) authenticate(password);
    else openCredentials();
  }
  function simulateApproval() { state.session = 'view'; state.hostPhase = 'view'; state.controlGranted = false; state.page = 'session'; state.controlRequested = false; record('批准查看屏幕'); render(); }
  function changeVariant(v) { state.variant = v; state.page = 'devices'; state.role = 'controller'; render(); }

  function handleAction(action) {
    if (action.startsWith('role-')) {
      closeDialog();
      state.role = action.slice(5);
      if (state.role === 'host') state.hostTab = 'home';
      if (state.role === 'controller') state.page = state.session === 'idle' ? 'devices' : 'session';
      render(); return;
    }
    if (action.startsWith('access-')) { state.currentAccessMode = action.slice(7); state.formError = ''; render(); return; }
    if (action === 'use-attended') { state.currentAccessMode = 'attended'; openAttendedCredentials(); return; }
    if (action === 'demo-password') { const field = $('dialog-password') || $('access-password'); if (field) { field.value = deviceAccess(selectedDevice()).password; field.focus(); } return; }
    if (action === 'toggle-password') { const field = $('dialog-password') || $('access-password'); if (field) field.type = field.type === 'password' ? 'text' : 'password'; return; }
    if (action.startsWith('viewport-')) { state.viewport = action.slice(9); state.landscape = state.viewport !== 'phone'; render(); return; }
    if (action === 'rotate') { state.landscape = !state.landscape; render(); return; }
    if (action.startsWith('variant-')) { const dir = action.slice(8); changeVariant(['A', 'B', 'C'].includes(dir) ? dir : ['A', 'B', 'C'][(['A', 'B', 'C'].indexOf(state.variant) + (dir === 'next' ? 1 : 2)) % 3]); return; }
    if (action.startsWith('page-')) { state.page = action.slice(5); render(); return; }
    if (action.startsWith('mode-')) { state.connectMode = action.slice(5); state.formError = ''; render(); return; }
    if (action.startsWith('filter-')) { state.filter = action.slice(7); render(); return; }
    if (action.startsWith('select-')) { state.selected = action.slice(7); render(); return; }
    if (action.startsWith('connect-')) { startConnect(action.slice(8)); return; }
    if (action === 'close-dialog') { closeDialog(); return; }
    if (action === 'open-host') { closeDialog(); state.role = 'host'; state.hostTab = 'home'; render(); return; }
    if (action === 'simulate-invite') { window.HostPrototype.handle('enable', state, helpers); openCredentials(); return; }
    if (action === 'add-device') { dialog('<h2>添加常用设备</h2><p>保存设备信息，方便下次找到它。保存不会发起连接。</p><form id="add-form"><label for="new-name">设备名称</label><input id="new-name" name="name" placeholder="例如：家里的电脑" maxlength="24" required><label for="new-id">设备 ID</label><input id="new-id" name="id" placeholder="输入 9 位设备 ID" inputmode="numeric" required><p class="error-inline" id="add-error" role="alert"></p><button type="submit" class="btn primary block">保存设备</button></form>'); return; }
    if (action === 'custom-connect') { changeVariant('A'); return; }
    if (action === 'design') { dialog(`<h2>把熟悉的桌面，带到手边。</h2><p>先评审双端信息层级、跨设备布局与授权体验。三种控制端结构共享同一套连接语义。</p><div class="design-options">${Object.entries(variants).map(([v, [name, desc]]) => `<section class="design-option ${state.variant === v ? 'active' : ''}"><span class="eyebrow">DIRECTION ${v}</span><h3>${name}${v === 'A' ? ' · 推荐' : ''}</h3><p>${desc}。${v === 'A' ? '适合多设备的日常连接，手机压缩为单列。' : v === 'B' ? '减少首次使用负担，设备管理密度较低。' : 'PC 与平板信息利用率高，手机采用紧凑主从结构。'}</p></section>`).join('')}</div><div class="design-adapt"><b>手机</b>　单列 / 桌面画面 / 按需触控板<br><b>平板</b>　导航轨 / 桌面画面 / 浮动工具<br><b>PC</b>　常驻侧栏 / 设备工作台 / 桌面画面 / 浮动工具栏</div><div class="divider"></div><p>无人值守：设备 ID＋访问密码 → 直接进入桌面；临时协助：单次协助码＋现场批准。会话支持仅查看／控制，文件上传与下载独立授权。</p><a href="DESIGN.md" target="_blank" rel="noopener">阅读完整设计草案 ↗</a><p style="font-size:10px;margin-top:14px;margin-bottom:0">所有数据、桌面画面、身份结果和性能指标均为模拟。本原型不接入 POC，也不表示真实远控或鸿蒙系统 API 已经实现。</p>`, true); return; }
    if (action === 'preview-session') { if (state.session === 'idle' || !['view', 'control', 'pending'].includes(state.session)) { resetFileWorkspace(); state.selected = 'studio'; state.activeDevice = { ...selectedDevice() }; state.activeMode = effectiveConnectMode(); state.activeAccess = 'unattended'; state.controlGranted = true; state.session = 'control'; state.hostPhase = 'control'; state.filePermissions = { upload: false, download: false }; } state.page = 'session'; render(); notify('会话设计预览：画面与连接指标均为模拟'); return; }
    if (action === 'resume-session') { state.page = 'session'; render(); return; }
    if (action === 'session-back') { state.page = 'devices'; render(); return; }
    if (action === 'disconnect') { finishSession('ended'); return; }
    if (action === 'simulate-approve') { simulateApproval(); return; }
    if (action === 'simulate-reject') { finishSession('rejected'); return; }
    if (action === 'simulate-timeout') { finishSession('timeout'); return; }
    if (action === 'simulate-identity') { finishSession('identity'); return; }
    if (action === 'simulate-network') { finishSession('network'); return; }
    if (action === 'view-mode') { state.session = 'view'; state.keyboard = false; state.trackpad = false; state.event = ''; render(); return; }
    if (action === 'control-mode') { if (state.controlGranted) { state.session = 'control'; state.hostPhase = 'control'; state.controlRequested = false; render(); } else { handleAction('request-control'); } return; }
    if (action === 'collapse-toolbar') { state.toolbarCollapsed = !state.toolbarCollapsed; render(); return; }
    if (action === 'trackpad') { state.trackpad = !state.trackpad; state.keyboard = false; state.tools = false; render(); return; }
    if (action === 'files') { state.filesOpen = true; state.tools = false; state.keyboard = false; state.trackpad = false; render(); return; }
    if (action === 'close-files') { state.filesOpen = false; render(); return; }
    if (action === 'screen-fit' || action === 'screen-fill') { state.screenFit = action === 'screen-fill' ? 'fill' : 'fit'; render(); return; }
    if (action === 'request-control') { if (state.session !== 'view') return; state.controlRequested = true; state.event = ''; render(); notify('已请求操作权限，等待对方单独允许'); return; }
    if (action === 'cancel-control') { if (!state.controlRequested) return; state.controlRequested = false; render(); notify('已取消操作权限请求'); return; }
    if (action === 'simulate-grant') { if (state.session !== 'view') return; if (state.controlRequested) state.session = 'control'; state.hostPhase = 'control'; state.controlGranted = true; state.controlRequested = false; record('对方允许键盘与鼠标'); render(); notify('模拟：对方已允许操作'); return; }
    if (action === 'simulate-revoke') { state.controlGranted = false; state.trackpad = false; state.session = 'view'; state.hostPhase = 'view'; state.keyboard = false; state.controlRequested = false; state.event = ''; record('对方撤销操作权限'); render(); notify('操作已停止，当前仅可查看'); return; }
    if (action.startsWith('input-')) { state.inputMode = action.slice(6); render(); return; }
    if (action === 'keyboard') {
      if (state.session !== 'control' || !state.controlGranted) { notify('仅查看模式下无法输入'); return; }
      state.keyboard = true; state.remoteEditor = true; state.trackpad = false; state.tools = false; render(); remoteKeyboard.activate(); return;
    }
    if (action === 'keyboard-close') { state.keyboard = false; remoteKeyboard.stop(); render(); return; }
    if (action === 'tools') { state.tools = !state.tools; render(); return; }
    if (action === 'fit') { notify('画面已适应当前窗口'); return; }
    if (action.startsWith('key-')) { if (state.session !== 'control' || !state.controlGranted || !state.keyboard) return; remoteKeyboard.activate(); remoteKeyboard.command(action.slice(4)); return; }
    if (action === 'left-click' || action === 'right-click') { if (state.session !== 'control' || !state.controlGranted) return; state.event = action === 'left-click' ? '模拟输入：左键' : action === 'right-click' ? '模拟输入：右键' : `模拟快捷键：${action.slice(4)}`; render(); return; }
  }
  document.addEventListener('click', event => {
    const file = event.target.closest('[data-file-action]');
    if (file) { if (!file.disabled) window.FilePrototype.handle(file.dataset.fileAction, state, helpers); return; }
    const host = event.target.closest('[data-host-action]');
    if (host) { window.HostPrototype.handle(host.dataset.hostAction, state, helpers); return; }
    if (event.target.closest('.remote-display') && state.keyboard && state.session === 'control' && state.controlGranted) remoteKeyboard.activate();
    const target = event.target.closest('[data-action]');
    if (target && !target.disabled) handleAction(target.dataset.action);
    else if (event.target.classList.contains('overlay-backdrop')) closeDialog();
  });
  document.addEventListener('submit', event => {
    event.preventDefault();
    if (event.target.id === 'connect-form') {
      if (['pending', 'view', 'control'].includes(state.session)) { notify('当前已有会话，请先结束本次连接'); state.page = 'session'; render(); return; }
      const form = new FormData(event.target);
      if (state.connectMode === 'ip') {
        state.ip = String(form.get('ip')).trim(); state.port = String(form.get('port')).trim();
        let validIP = false;
        if (/^(?:\d{1,3}\.){3}\d{1,3}$/.test(state.ip)) validIP = state.ip.split('.').every(part => Number(part) <= 255);
        else if (state.ip.includes(':')) { try { new URL(`http://[${state.ip.replace(/^\[|\]$/g, '')}]/`); validIP = true; } catch { validIP = false; } }
        if (!validIP || !/^\d+$/.test(state.port) || Number(state.port) < 1 || Number(state.port) > 65535) { state.formError = '请填写有效的 IPv4 / IPv6 地址和 1–65535 端口。'; render(); return; }
        state.selected = 'studio';
      } else {
        state.deviceId = String(form.get('deviceId')).trim();
        if (!/^\d{9}$/.test(state.deviceId.replace(/\s/g, ''))) { state.formError = '请填写 9 位设备 ID。'; render(); return; }
        const target = state.devices.find(d => d.deviceId.replace(/\s/g, '') === state.deviceId.replace(/\s/g, ''));
        if (!target) { state.formError = '演示中未找到此设备，可使用 842 916 305。'; render(); return; }
        state.selected = target.id;
      }
      state.formError = ''; startConnect(undefined, (state.currentAccessMode || state.defaultAccessMode) === 'unattended' ? String(form.get('password') || '') : undefined);
    }
    if (event.target.id === 'password-form') {
      if (['pending', 'view', 'control'].includes(state.session)) { closeDialog(); notify('已有活动会话'); return; }
      authenticate($('dialog-password').value, 'password-error');
    }
    if (event.target.id === 'credential-form') {
      if (state.hostPhase !== 'inviting') { closeDialog(); notify('临时邀请已失效，请让对方重新开启协助'); return; }
      if ($('assist-code').value.replace(/\s/g, '') !== '728461') { $('credential-error').textContent = '协助码不正确。原型示例为 728 461。'; return; }
      resetFileWorkspace(); closeDialog(); state.activeDevice = { ...selectedDevice() }; state.activeMode = effectiveConnectMode(); state.activeAccess = 'attended'; state.controlGranted = false; state.filePermissions = { upload: false, download: false }; state.session = 'pending'; state.hostPhase = 'pending'; state.hostTab = 'home'; state.hostAccessTab = 'attended'; state.connectRequestedMode = null; state.page = 'session'; state.event = ''; state.keyboard = false; state.controlRequested = false; record('发起查看请求'); render();
    }
    if (event.target.id === 'add-form') {
      const form = new FormData(event.target); const name = String(form.get('name')).trim(); const id = String(form.get('id')).replace(/\s/g, '');
      if (!name || !/^\d{9}$/.test(id)) { $('add-error').textContent = '请填写设备名称和 9 位设备 ID。'; return; }
      if (state.devices.some(d => d.deviceId.replace(/\s/g, '') === id)) { $('add-error').textContent = '这台设备已经在列表中了。'; return; }
      state.devices.push({ id: 'device-' + id, name, deviceId: id.replace(/(\d{3})(\d{3})(\d{3})/, '$1 $2 $3'), os: 'Windows', online: false, color: 'gray', subtitle: '尚未连接', tag: '个人设备', unattended: { enabled: false, trusted: false, password: '', control: false, upload: false, download: false } });
      closeDialog(); render(); notify('设备信息已保存在本次原型中');
    }
  });
  document.addEventListener('input', event => {
    if (event.target.id === 'search') { state.search = event.target.value; $('device-grid').innerHTML = visibleDevices().map(deviceCard).join('') || '<p class="empty">没有找到匹配的设备</p>'; }
    const fieldMap = { 'device-id-input': 'deviceId', 'ip-input': 'ip', 'port-input': 'port' };
    if (fieldMap[event.target.id]) state[fieldMap[event.target.id]] = event.target.value;
  });
  document.addEventListener('change', event => {
    if (event.target.id === 'requested-mode') state.connectRequestedMode = event.target.value;
    if (event.target.id === 'setting-mode') state.defaultRequestedMode = event.target.value;
    if (event.target.id === 'setting-access') { state.defaultAccessMode = event.target.value; state.currentAccessMode = null; notify('默认访问方式已更新，将用于下一次连接'); }
    if (event.target.id === 'setting-policy') { state.connectionPolicy = event.target.value; notify('连接策略已更新，将用于下一次连接'); }
    if (event.target.id === 'setting-fit') { state.screenFit = event.target.value; notify('画面显示偏好已更新'); }
    if (event.target.id === 'quality' || event.target.id === 'setting-quality') { state.quality = event.target.value; notify('画质偏好已更新'); }
  });
  document.addEventListener('keydown', event => {
    if (event.target.id === 'remote-input-sink' || event.isComposing || event.keyCode === 229 || remoteKeyboard.isComposing()) return;
    if (event.key === 'Escape') { if ($('overlay').children.length) closeDialog(); else if (state.filesOpen || state.keyboard || state.tools || state.trackpad) { state.filesOpen = false; state.keyboard = false; state.tools = false; state.trackpad = false; render(); } return; }
    if ($('overlay').children.length || event.target.closest('input,textarea,select,[contenteditable],button') || state.page === 'session') return;
    if (['ArrowLeft', 'ArrowRight'].includes(event.key) && state.role === 'controller') { event.preventDefault(); handleAction(event.key === 'ArrowRight' ? 'variant-next' : 'variant-prev'); }
  });
  function bindPointer() {
    document.querySelectorAll('.touchpad,.remote-display').forEach(el => {
      el.addEventListener('pointermove', event => {
        if (state.session !== 'control' || !state.controlGranted || event.buttons !== 1) return;
        if (el.classList.contains('remote-display') && state.inputMode !== 'touch' && state.viewport !== 'pc') return;
        const rect = el.getBoundingClientRect(); const cursor = $('remote-cursor');
        if (cursor) { cursor.style.left = Math.max(1, Math.min(96, (event.clientX - rect.left) / rect.width * 100)) + '%'; cursor.style.top = Math.max(1, Math.min(92, (event.clientY - rect.top) / rect.height * 100)) + '%'; }
      });
      el.addEventListener('pointerdown', event => { if (state.session === 'control' && state.controlGranted) el.setPointerCapture(event.pointerId); });
    });
  }
  render();
})();
