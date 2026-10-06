(function () {
  'use strict';

  const defaultDevice = { id: 'studio', name: '工作室电脑', deviceId: '842 916 305', os: 'Windows 11' };
  const invitationCode = '728 461';
  const phases = {
    off: ['远程访问已关闭', '尚未开启协助', 'neutral'],
    inviting: ['等待对方连接', '邀请有效 · 10 分钟', 'ready'],
    pending: ['有新的协助请求', '等待你的批准', 'pending'],
    view: ['正在共享屏幕', '对方仅可查看', 'ready'],
    control: ['正在接受远程协助', '对方可查看和操作', 'ready'],
    ended: ['本次协助已结束', '远程访问已关闭', 'neutral']
  };

  function selectedHostDevice(state) {
    return (state.devices || []).find(device => device.id === state.selected) ||
      (state.devices || []).find(device => device.id === 'studio') || defaultDevice;
  }

  function targetHostDevice(state) {
    return ['pending', 'view', 'control'].includes(state.session) && state.activeDevice
      ? state.activeDevice
      : selectedHostDevice(state);
  }

  function render(state, helpers) {
    const icon = helpers.icon;
    const escape = helpers.escape;
    const phase = phases[state.hostPhase] ? state.hostPhase : 'off';
    const active = phase === 'view' || phase === 'control';
    const selectedProfile = targetHostDevice(state).unattended;
    const unattended = selectedProfile || state.unattended || (state.unattended = { enabled: false, trusted: false, password: '', control: false, upload: false, download: false });
    state.unattended = unattended;
    const granted = active && state.controlGranted;
    const info = !active && ['off', 'ended'].includes(phase) && unattended.enabled && unattended.trusted
      ? ['无人值守已开启 · 等待密码连接', '已配置 · 可远程访问', 'ready']
      : !active && ['off', 'ended'].includes(phase) && unattended.enabled
        ? ['无人值守已开启 · 等待登记', '暂无可信控制端，密码连接会被拒绝', 'pending']
        : active ? [state.session === 'control' ? '正在接受远程操作' : '正在共享屏幕', state.session === 'control' ? '对方处于控制模式' : '对方处于仅查看模式', 'ready'] : phases[phase];
    const tab = state.hostTab || 'home';
    const accessTab = state.hostAccessTab || 'unattended';
    const hasSessionDevice = (active || phase === 'pending') && state.activeDevice;
    const device = hasSessionDevice ? state.activeDevice : selectedHostDevice(state);
    const connectionMode = hasSessionDevice ? (state.activeMode || state.connectMode) : state.connectMode;
    const connectionPath = connectionMode === 'ip' ? 'IP 直连' : connectionMode === 'relay' ? '加密中继' : '协调直连';
    const button = (action, label, kind, name) => `<button type="button" class="host-button ${kind || ''}" data-host-action="${action}">${name ? icon(name, 18) : ''}<span>${label}</span></button>`;
    const badge = `<span class="host-badge ${info[2]}"><i></i>${info[1]}</span>`;
    const toggle = (action, label, checked) => `<button type="button" class="host-switch ${checked ? 'on' : ''}" role="switch" aria-checked="${!!checked}" aria-label="${label}" data-host-action="${action}"><span></span></button>`;

    function unattendedCard() {
      return `<section class="host-card host-unattended-card">
        <div class="host-section-heading"><div class="host-eyebrow">${icon('lock', 16)}无人值守访问</div><span class="host-setting-pill">已配置的演示设备</span></div>
        <h2>不在电脑旁，也能连接。</h2><p class="host-body-copy">${unattended.trusted ? '已登记的控制端使用本机 ID 与访问密码验证，通过后直接进入桌面，无需每次在现场批准。' : '登记可信控制设备后，才能使用无人值守密码连接。'}</p>
        <div class="host-trusted-controller">${icon('shield', 15)}<span>${unattended.trusted ? '已登记的控制端：我的控制设备 · 示例' : '当前控制端未登记，密码连接会被拒绝'}</span></div>
        <div class="host-permission-row host-trusted-row"><div class="host-permission-icon">${icon('shield', 19)}</div><div><strong>可信控制设备</strong><span>${unattended.trusted ? '我的控制设备 · 示例已登记' : '未登记，不能使用无人值守连接'}</span></div>${toggle('toggle-trust', '登记的控制设备', unattended.trusted)}</div>
        <div class="host-unattended-toggle"><div><strong>开启无人值守</strong><span>${unattended.enabled && unattended.trusted ? '等待密码验证，可随时关闭' : unattended.enabled ? '已开启，但暂无可信控制端' : '已关闭，ID 与密码暂不可连接'}</span></div>${toggle('toggle-unattended', '开启无人值守访问', unattended.enabled)}</div>
        <div class="host-invitation host-access-credentials"><div class="host-credential"><span>本机设备 ID</span><div><strong>${escape(device.deviceId)}</strong><button type="button" class="host-icon-button" aria-label="复制设备 ID" data-host-action="copy-id">${icon('copy', 17)}</button></div></div><div class="host-credential"><span>访问密码 · ${unattended.password ? '已设置' : '未设置'}</span><div><strong class="host-password-display">${state.hostPasswordVisible ? escape(unattended.password) : '••••••••'}</strong>${button('show-password', state.hostPasswordVisible ? '隐藏' : '显示', 'text')}</div></div><div class="host-password-actions">${button('edit-password', state.hostPasswordEditing ? '取消修改' : '修改演示密码', 'text', 'lock')}<span>仅用于此设计稿</span></div></div>
        ${state.hostPasswordEditing ? `<div class="host-password-editor"><label for="host-demo-password">模拟新密码（至少 8 个字符）</label><input id="host-demo-password" type="password" placeholder="模拟输入，请勿填写真实密码" autocomplete="off" minlength="8" maxlength="64" aria-describedby="host-password-note"><p id="host-password-note">仅更新内存中的演示密码，刷新即重置。</p>${button('save-password', '保存演示密码', 'primary', 'check')}</div>` : ''}
        <div class="host-policy-heading"><h3>密码连接后的默认权限</h3><span>下次连接应用；当前会话可单独调整</span></div>
        <div class="host-permission-row"><div class="host-permission-icon">${icon('monitor', 19)}</div><div><strong>允许查看</strong><span>通过密码验证后可查看主屏幕</span></div><span class="host-value enabled">已允许</span></div>
        <div class="host-permission-row"><div class="host-permission-icon">${icon('mouse', 19)}</div><div><strong>允许控制</strong><span>可切换到键盘与鼠标操作</span></div>${toggle('policy-control', '无人值守允许控制', unattended.control)}</div>
        <div class="host-permission-row"><div class="host-permission-icon">${icon('folder', 19)}</div><div><strong>允许上传到本机</strong><span>仅接收至「远程共享」目录</span></div>${toggle('policy-upload', '无人值守允许上传文件到本机', unattended.upload)}</div>
        <div class="host-permission-row"><div class="host-permission-icon">${icon('folder', 19)}</div><div><strong>允许从本机下载</strong><span>仅开放「远程共享」目录内文件</span></div>${toggle('policy-download', '无人值守允许下载本机文件', unattended.download)}</div>
        <div class="host-shared-directory">${icon('folder', 17)}<div><strong>文件共享范围</strong><span>此电脑 / 文档 / 远程共享</span></div><span>固定演示目录</span></div>
        <p class="host-inline-note">${icon('info', 15)}<span>演示已完成预配置；首次安装默认关闭，由本机设置密码并开启。屏幕控制与文件权限分别授权。</span></p>
      </section>`;
    }

    function accessSettings() {
      return `<div class="host-access-tabs" role="tablist" aria-label="本机访问方式"><button type="button" role="tab" aria-selected="${accessTab === 'unattended'}" class="${accessTab === 'unattended' ? 'active' : ''}" data-host-action="access-unattended">${icon('lock', 17)}<span>无人值守<small>ID 与密码连接</small></span></button><button type="button" role="tab" aria-selected="${accessTab === 'attended'}" class="${accessTab === 'attended' ? 'active' : ''}" data-host-action="access-attended">${icon('user', 17)}<span>临时协助<small>单次邀请 · 现场批准</small></span></button></div>${accessTab === 'unattended' ? unattendedCard() : active || phase === 'pending' ? `<section class="host-card host-attended-summary"><div class="host-eyebrow">${icon('user', 16)}临时协助</div><h3>当前连接流程结束后，可开启新的临时邀请。</h3><p class="host-body-copy">临时协助使用单次协助码，每次连接均需在本机批准查看。</p></section>` : mainCard()}`;
    }

    function invitation() {
      return `<div class="host-invitation">
        <div class="host-section-heading"><span>本次协助邀请</span><span class="host-small-note">单次使用</span></div>
        <div class="host-credential"><span>本机设备 ID</span><div><strong>${escape(device.deviceId)}</strong><button type="button" class="host-icon-button" aria-label="复制设备 ID" data-host-action="copy-id">${icon('copy', 17)}</button></div></div>
        <div class="host-credential"><span>临时协助码</span><div><strong>${invitationCode}</strong><button type="button" class="host-icon-button" aria-label="复制临时协助码" data-host-action="copy-code">${icon('copy', 17)}</button></div></div>
        <p class="host-inline-note">${icon('clock', 15)}<span>10 分钟内有效。把邀请发给你认识的人。</span></p>
        ${button('copy-invite', '复制协助邀请', 'secondary', 'copy')}
      </div>`;
    }

    function mainCard() {
      if (phase === 'off' || phase === 'ended') {
        const ended = phase === 'ended';
        return `<section class="host-card host-start-card">
          <div class="host-eyebrow">${icon('shield', 16)}由你决定何时连接</div>
          <div class="host-computer-art" aria-hidden="true"><div class="host-monitor"><div class="host-monitor-top"><i></i><i></i><i></i></div><div class="host-monitor-shield">${icon(ended ? 'check' : 'lock', 33)}</div><div class="host-monitor-line"></div><div class="host-monitor-line short"></div></div><div class="host-monitor-neck"></div><div class="host-monitor-base"></div><div class="host-art-orbit">${icon('shield', 22)}</div></div>
          <h2>${ended ? '连接结束，安心继续。' : '需要帮忙时，再开启协助。'}</h2>
          <p>${ended ? escape(state.hostEndReason || '画面共享与远程操作均已停止。') : '生成一次临时邀请，让你认识的人连接这台电脑。每次连接都需要你在这里批准。'}</p>
          ${button('enable', ended ? '发起新的协助' : '开启临时协助', 'primary large', 'power')}
          <div class="host-start-foot">${icon('check', 15)}仅本次有效 ${icon('check', 15)}随时可以结束</div>
        </section>`;
      }
      if (phase === 'inviting') {
        return `<section class="host-card host-invite-card"><div class="host-eyebrow">${icon('link', 16)}临时协助已开启</div><h2>把邀请交给信任的人。</h2><p class="host-body-copy">对方输入设备 ID 和协助码后，你会收到连接请求。批准之前，不会共享画面。</p>${invitation()}<div class="host-card-footer">${icon('shield', 16)}<span>关闭协助后，本次邀请立即失效。</span>${button('stop-invite', '关闭协助', 'text')}</div></section>`;
      }
      if (phase === 'pending') {
        return `<section class="host-card host-request-card"><div class="host-request-top"><span class="host-eyebrow">${icon('user', 16)}请确认连接请求</span><span class="host-timer">${icon('clock', 15)}等待上限 60 秒</span></div><h2>“我的平板”想查看你的屏幕</h2><p class="host-body-copy">请先确认这是你正在邀请的人，再允许对方查看。</p><div class="host-request-identity"><div class="host-avatar">${icon('user', 24)}</div><div><strong>我的平板</strong><span>名称由对方提供 · 控制设备身份尚未验证</span></div></div><div class="host-pair-code"><span>与对方核对配对码</span><strong>492 816</strong><small>双方显示相同数字后，再批准连接</small></div><div class="host-request-scope"><span>${icon('monitor', 19)}请求查看你的屏幕</span><strong>仅查看</strong><p>对方暂时不能使用键盘和鼠标。连接后，你可以单独允许操作。</p></div><div class="host-button-row">${button('reject', '拒绝', 'secondary')}${button('approve', '批准查看', 'primary', 'check')}</div><p class="host-inline-note">${icon('lock', 15)}<span>设备连接已加密；配对核对不代表长期信任。</span></p></section>`;
      }
      return `<section class="host-card host-session-card"><div class="host-section-heading"><div class="host-eyebrow">${icon('activity', 16)}当前远程连接</div><span class="host-connected"><i></i>连接中</span></div><h2>正在与“我的控制设备”连接</h2><p class="host-body-copy">${state.session === 'control' ? '对方正在操作你的桌面。你可以随时撤销操作权限，或结束会话。' : granted ? '对方选择仅查看；操作权限已授予，可以自行切换控制模式。' : '对方正在查看你的屏幕，键盘和鼠标仍由你掌控。'}</p><div class="host-preview"><div class="host-preview-title"><span>${icon('monitor', 15)}${escape(device.name)} · 主显示器</span><span>屏幕共享中</span></div><div class="host-mini-desktop">${state.remoteEditor === true ? `<div class="host-preview-notepad"><div class="host-notepad-title">${icon('file', 13)}<span>记事本 · 输入演示</span><small>只读预览</small></div><pre class="host-notepad-text" tabindex="0" aria-label="被控端收到的演示文字">${state.remoteText ? escape(state.remoteText) : '<span class="host-notepad-placeholder">等待控制端输入…</span>'}</pre></div>` : '<div class="host-mini-window"><div class="host-mini-window-top"><i></i><i></i><i></i></div><div class="host-mini-window-body"><div class="host-mini-sidebar"></div><div class="host-mini-document"><span></span><span></span><span></span><b></b><span></span></div></div></div>'}<div class="host-mini-dock"><i></i><i></i><i></i><i></i></div><div class="host-preview-label">共享范围：整个主屏幕</div></div></div><div class="host-live-person"><div class="host-avatar small">${icon('user', 19)}</div><div><strong>我的控制设备</strong><span>${state.activeAccess === 'unattended' ? '无人值守 · 密码验证' : '临时协助 · 本次批准'} · ${state.session === 'control' ? '控制模式' : '仅查看模式'}</span></div><div class="host-path">${icon('link', 14)}${connectionPath}</div></div><div class="host-session-bottom"><span>${icon('lock', 15)}已加密连接</span>${button('disconnect', '结束本次连接', 'danger', 'close')}</div></section>`;
    }

    function permissions() {
      const filePermissions = state.filePermissions || {};
      const transferRows = [['upload', '允许上传到本机', '写入本机「远程共享」目录'], ['download', '允许从本机下载', '读取本机「远程共享」目录']].map(([direction, label, detail]) => `<div class="host-permission-row"><div class="host-permission-icon">${icon('folder', 19)}</div><div><strong>${label}</strong><span>${active && state.fileRequests?.[direction] && !filePermissions[direction] ? `对方请求${direction === 'upload' ? '上传' : '下载'}，等待你允许` : detail}</span></div>${active ? toggle(`file-${direction}`, `本次会话${label}`, filePermissions[direction]) : '<span class="host-value">无活动会话</span>'}</div>`).join('');
      return `<section class="host-card host-permissions"><div class="host-section-heading"><h3>本次会话权限</h3>${icon('shield', 19)}</div><p class="host-muted">${active ? '修改立即生效，结束会话后收回本次授权。' : '连接后，可在此单独调整本次权限。'}</p><div class="host-permission-row"><div class="host-permission-icon">${icon('monitor', 19)}</div><div><strong>查看屏幕</strong><span>${active ? '主显示器正在共享' : '验证或批准通过后才能查看'}</span></div><span class="host-value ${active ? 'enabled' : ''}">${active ? '已允许' : '未共享'}</span></div><div class="host-permission-row"><div class="host-permission-icon">${icon('mouse', 19)}</div><div><strong>键盘与鼠标</strong><span>${granted ? state.session === 'control' ? '对方当前处于控制模式' : '已授权，对方选择仅查看' : active ? state.controlRequested ? '对方请求控制，等待你允许' : '需要你单独允许' : '等待连接后确认'}</span></div>${active ? toggle(granted ? 'revoke' : 'grant', '本次会话允许键盘与鼠标操作', granted) : '<span class="host-value">未允许</span>'}</div>${active ? `<div class="host-permission-explain ${granted ? 'control' : ''}">${icon(granted ? 'info' : 'lock', 17)}<span>${granted ? '允许对方切换控制模式。关闭开关即可立即撤销操作，继续仅查看。' : '查看与操作独立授权；允许操作后，才可点击、输入或打开应用。'}</span></div>` : ''}${transferRows}<div class="host-permission-explain">${icon('folder', 17)}<span>文件传输独立于屏幕控制。撤销上传或下载时，立即停止对应方向的传输。</span></div><div class="host-closed-capabilities"><span>尚未开放</span><div><span>${icon('lock', 13)}剪贴板</span><span>${icon('lock', 13)}远程终端</span><span>${icon('lock', 13)}管理员提权</span></div></div></section>`;
    }

    function boundaries() {
      return `<section class="host-card host-boundaries"><div class="host-section-heading"><h3>连接方式，各有边界</h3>${icon('lock', 18)}</div><div><span class="host-boundary-check">${icon('check', 15)}</span><p><strong>临时协助，现场批准</strong><span>单次邀请 10 分钟有效，批准前不会共享画面。</span></p></div><div><span class="host-boundary-check">${icon('check', 15)}</span><p><strong>无人值守，密码验证</strong><span>${unattended.enabled && unattended.trusted ? '已开启。通过密码验证后，按预设权限连接。' : unattended.enabled ? '已开启，但尚无可信控制端，密码连接会被拒绝。' : '已关闭。需本机设置密码并主动开启。'}</span></p></div><div><span class="host-boundary-check">${icon('check', 15)}</span><p><strong>结束即停止本次共享</strong><span>画面、操作和传输同时结束。已开启的无人值守策略保留，可单独关闭。</span></p></div></section>`;
    }

    function events() {
      const entries = state.hostEvents || [{ text: unattended.enabled ? '演示设备已配置无人值守访问' : '远程访问保持关闭', time: '现在' }];
      return `<section class="host-card host-events"><div class="host-section-heading"><h3>本次活动</h3><span class="host-small-note">只记录连接与授权</span></div>${entries.slice().reverse().map(event => `<div class="host-event"><span class="host-event-dot"></span><span>${escape(event.text)}</span><time>${escape(event.time)}</time></div>`).join('')}<p class="host-events-note">不记录画面、输入文字、访问密码或临时协助码。</p></section>`;
    }

    function simulation() {
      let controls = '';
      if (phase === 'inviting') controls = button('simulate-request', '收到连接请求', 'simulation', 'user') + button('expire-invite', '邀请到期', 'simulation', 'clock');
      if (phase === 'pending') controls = button('request-timeout', '批准等待超时', 'simulation', 'clock');
      if (active) controls = button('remote-disconnect', '对方断开连接', 'simulation', 'link');
      return `<div class="host-simulation"><span>${icon('info', 15)}原型演示</span><p>${controls ? '模拟对方或时间触发的事件' : '点击上方按钮，体验本机协助流程'}</p><div>${controls}</div></div>`;
    }

    const title = tab === 'records' ? '访问记录' : tab === 'settings' ? '安全设置' : '本机远程访问';
    const body = tab === 'records' ? `<div class="host-records-layout">${events()}${boundaries()}</div>` : `<div class="host-grid"><div class="host-left-column">${active || phase === 'pending' ? mainCard() : ''}${accessSettings()}${tab === 'home' ? events() : ''}</div><div class="host-right-column">${permissions()}${boundaries()}</div></div>`;

    return `<div class="host-root"><aside class="host-sidebar"><div class="host-brand"><span>${icon('monitor', 23)}</span><div><strong>Open Remote Desk</strong><small>被控端</small></div></div><div class="host-nav-label">工作空间</div><nav aria-label="被控端导航">${[['home', 'monitor', '本机访问'], ['records', 'activity', '访问记录'], ['settings', 'shield', '安全设置']].map(item => `<button type="button" class="host-nav-item ${tab === item[0] ? 'active' : ''}" data-host-action="tab-${item[0]}" aria-label="${item[2]}" ${tab === item[0] ? 'aria-current="page"' : ''}>${icon(item[1], 19)}<span>${item[2]}</span>${tab === item[0] ? '<i></i>' : ''}</button>`).join('')}</nav><div class="host-sidebar-bottom"><div class="host-local-machine">${icon('monitor', 19)}<div><strong>${escape(device.name)}</strong><span>${escape(device.os || "Windows")} · 本机</span></div></div><div class="host-sidebar-version">Open Remote Desk <span>设计预览</span></div></div></aside><main class="host-main"><header class="host-header"><div><span class="host-overline">你的电脑 · 你的决定</span><h1>${title}</h1></div><span class="host-device-pill">${icon('monitor', 17)}Windows 被控端</span></header><div class="host-status-bar"><div class="host-status-icon ${active ? 'live' : ''}">${icon(active ? 'monitor' : phase === 'pending' ? 'user' : 'shield', 21)}</div><div><strong>${info[0]}</strong><span>${active ? '你可以随时撤销操作权限，或结束整个会话。' : phase === 'pending' ? '批准前，屏幕和输入权限保持关闭。' : phase === 'inviting' ? unattended.enabled && unattended.trusted ? '临时邀请等待批准；已开启的无人值守仍可通过密码连接。' : unattended.enabled ? '临时邀请等待批准；无人值守尚无可信控制端。' : '等待连接时，你的屏幕仍然不会被共享。' : unattended.enabled && unattended.trusted ? '已登记的控制端可凭 ID 与访问密码连接，按预设权限访问。' : unattended.enabled ? '无人值守已开启，但暂无可信控制端，密码连接会被拒绝。' : '无人值守和临时协助均已关闭。'}</span></div>${badge}</div>${body}${simulation()}</main></div>`;
  }

  function handle(action, state, helpers) {
    const phase = state.hostPhase || 'off';
    const active = phase === 'view' || phase === 'control';
    const selectedProfile = targetHostDevice(state).unattended;
    const unattended = selectedProfile || state.unattended || (state.unattended = { enabled: false, trusted: false, password: '', control: false, upload: false, download: false });
    state.unattended = unattended;
    const addEvent = text => {
      const time = new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
      state.hostEvents = (state.hostEvents || []).concat({ text, time }).slice(-6);
      const device = targetHostDevice(state);
      state.records = (state.records || []).concat({ text: `被控端：${text}`, time, name: device.name, path: '被控端设置' }).slice(-40);
    };
    const end = (reason, notice, session = 'ended') => {
      helpers.clearSession(session);
      state.hostPhase = 'ended';
      state.session = session;
      state.controlGranted = false;
      state.controlRequested = false;
      state.keyboard = false;
      state.event = '';
      state.hostEndReason = reason;
      addEvent(notice);
      helpers.notify(notice);
    };
    if (action.startsWith('tab-')) {
      const tab = action.slice(4);
      if (['home', 'records', 'settings'].includes(tab)) state.hostTab = tab;
    } else if (action === 'access-unattended' || action === 'access-attended') {
      state.hostAccessTab = action.slice(7);
    } else if (action === 'show-password') {
      state.hostPasswordVisible = !state.hostPasswordVisible;
    } else if (action === 'edit-password') {
      state.hostPasswordEditing = !state.hostPasswordEditing;
    } else if (action === 'save-password') {
      const input = document.getElementById('host-demo-password');
      const password = input ? input.value : '';
      if (password.length < 8 || password.length > 64) {
        helpers.notify('请输入 8–64 个字符的演示密码，请勿使用真实密码');
        if (input) input.focus();
        return;
      }
      unattended.password = password;
      state.hostPasswordEditing = false;
      state.hostPasswordVisible = false;
      addEvent('更新演示访问密码，下次连接需使用新密码');
      helpers.notify('演示密码已更新，下次连接生效');
    } else if (action === 'toggle-trust') {
      unattended.trusted = !unattended.trusted;
      if (!unattended.trusted && state.activeAccess === 'unattended' && active) {
        end('本机已撤销该控制设备的信任，本次无人值守连接已结束。', '撤销控制设备信任并结束当前连接');
      } else {
        addEvent(unattended.trusted ? '登记示例控制设备' : '撤销示例控制设备信任');
        helpers.notify(unattended.trusted ? '示例控制设备已登记' : '已撤销示例控制设备信任');
      }
    } else if (action === 'toggle-unattended') {
      if (!unattended.enabled && !unattended.trusted) {
        helpers.notify('先登记可信控制设备，再开启无人值守');
      } else if (!unattended.enabled && !unattended.password) {
        state.hostPasswordEditing = true;
        helpers.notify('先设置演示访问密码，再开启无人值守');
      } else {
        unattended.enabled = !unattended.enabled;
        if (!unattended.enabled && state.activeAccess === 'unattended' && active) {
          end('本机已关闭无人值守，本次画面、操作与文件传输均已停止。', '关闭无人值守并结束当前连接');
        } else {
          addEvent(unattended.enabled ? '开启无人值守，等待已登记控制端的密码连接' : '关闭无人值守访问');
          helpers.notify(unattended.enabled ? '无人值守已开启' : '无人值守已关闭');
        }
      }
    } else if (action.startsWith('policy-')) {
      const permission = action.slice(7);
      if (['control', 'upload', 'download'].includes(permission)) {
        unattended[permission] = !unattended[permission];
        if (!unattended[permission] && active && state.activeAccess === 'unattended') {
          if (permission === 'control') {
            state.controlGranted = false;
            state.hostPhase = 'view';
            state.session = 'view';
            state.keyboard = false;
            state.controlRequested = false;
            state.event = '';
          } else {
            state.filePermissions = { ...(state.filePermissions || {}), [permission]: false };
            helpers.stopTransfers('本机撤销无人值守文件权限', permission);
          }
        }
        const label = { control: '控制', upload: '上传至本机', download: '从本机下载' }[permission];
        addEvent(`无人值守策略：${unattended[permission] ? '允许' : '撤销'}${label}`);
        helpers.notify(unattended[permission] ? `下次密码连接将允许${label}` : `已撤销无人值守${label}权限`);
      }
    } else if ((action === 'file-upload' || action === 'file-download') && active) {
      const direction = action.slice(5);
      state.filePermissions = state.filePermissions || { upload: false, download: false };
      state.filePermissions[direction] = !state.filePermissions[direction];
      const permitted = state.filePermissions[direction];
      if (permitted && state.fileRequests) state.fileRequests[direction] = false;
      if (!permitted) helpers.stopTransfers('本机撤销本次文件权限', direction);
      const label = direction === 'upload' ? '上传至本机' : '从本机下载';
      addEvent(`${permitted ? '本次允许' : '本次撤销'}${label}`);
      helpers.notify(`${permitted ? '已允许' : '已撤销'}${label}${permitted ? '，共享范围仅限「远程共享」目录' : '，对应方向的传输已停止'}`);
    } else if (action === 'enable' && (phase === 'off' || phase === 'ended')) {
      state.hostAccessTab = 'attended';
      state.hostPhase = 'inviting';
      state.session = 'idle';
      state.hostEndReason = '';
      state.activeDevice = null;
      state.activeMode = null;
      state.activeAccess = null;
      state.controlGranted = false;
      state.filePermissions = { upload: false, download: false };
      state.controlRequested = false;
      state.keyboard = false;
      state.event = '';
      addEvent('开启临时协助，生成单次邀请');
      helpers.notify('临时协助已开启，请将邀请交给你认识的人');
    } else if (action === 'simulate-request' && phase === 'inviting') {
      helpers.resetFileWorkspace();
      state.activeDevice = { ...selectedHostDevice(state) };
      state.activeMode = state.activeMode || state.connectMode;
      state.activeAccess = 'attended';
      state.hostPhase = 'pending';
      state.session = 'pending';
      addEvent('收到“我的平板”的查看请求');
    } else if (action === 'approve' && phase === 'pending') {
      state.hostPhase = 'view';
      state.session = 'view';
      state.controlGranted = false;
      state.filePermissions = { upload: false, download: false };
      addEvent('批准查看屏幕，操作权限保持关闭');
      helpers.notify('已批准查看；键盘与鼠标尚未允许');
    } else if (action === 'grant' && active && !state.controlGranted) {
      state.hostPhase = 'control';
      state.controlGranted = true;
      if (state.controlRequested) state.session = 'control';
      state.controlRequested = false;
      addEvent('单独允许键盘与鼠标操作');
      helpers.notify('已允许对方使用控制模式，你可以随时关闭权限开关');
    } else if (action === 'revoke' && active && state.controlGranted) {
      state.hostPhase = 'view';
      state.session = 'view';
      state.controlGranted = false;
      state.controlRequested = false;
      state.keyboard = false;
      state.event = '';
      addEvent('撤销键盘与鼠标操作，保留屏幕查看');
      helpers.notify('操作权限已撤销，对方仅可查看');
    } else if (action === 'disconnect' && active) {
      end('你已结束本次连接。画面、操作和传输均已停止；无人值守策略保持原设置。', '本机结束连接，撤销本次全部授权');
    } else if (action === 'remote-disconnect' && active) {
      end('对方已断开连接。本次授权与文件传输已结束；无人值守策略保持原设置。', '原型演示：对方断开连接');
    } else if (action === 'reject' && phase === 'pending') {
      end('你已拒绝这次请求。屏幕未曾共享，本次邀请已失效。', '已拒绝连接请求', 'rejected');
    } else if (action === 'request-timeout' && phase === 'pending') {
      end('60 秒内未批准，请求已自动结束。屏幕没有被共享。', '原型演示：批准等待已超时', 'timeout');
    } else if (action === 'expire-invite' && phase === 'inviting') {
      end('临时邀请已超过 10 分钟有效期。需要协助时，请重新开启。', '原型演示：临时邀请已到期');
    } else if (action === 'stop-invite' && phase === 'inviting') {
      end('你已关闭临时协助。本次邀请已失效。', '已关闭临时协助');
    } else if (action === 'copy-id' || action.startsWith('copy-') && phase === 'inviting') {
      const deviceId = targetHostDevice(state).deviceId;
      const value = action === 'copy-id' ? deviceId : action === 'copy-code' ? invitationCode : `Open Remote Desk 临时协助（原型演示）\n设备 ID：${deviceId}\n协助码：${invitationCode}\n10 分钟有效，单次使用，连接后需本机批准。`;
      if (typeof navigator !== 'undefined' && navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(value).then(() => helpers.notify(action === 'copy-id' ? '已复制演示设备 ID' : '已复制演示邀请资料')).catch(() => helpers.notify(`请手动复制：${value}`));
      } else {
        helpers.notify(`请手动复制：${value}`);
      }
    }
    helpers.render();
  }

  window.HostPrototype = { render, handle };
}());
