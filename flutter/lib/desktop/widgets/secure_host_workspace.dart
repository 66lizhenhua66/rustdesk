import 'package:flutter/material.dart';

enum SecureHostStatus { pending, confirming, active, closing, ended }

@immutable
class SecureHostConnection {
  final int id;
  final String name;
  final String peerId;
  final SecureHostStatus status;
  final String? detail;
  final String? elapsedText;

  const SecureHostConnection({
    required this.id,
    required this.name,
    required this.peerId,
    required this.status,
    this.detail,
    this.elapsedText,
  });
}

@immutable
class SecureHostActivity {
  final String message;
  final String peerName;
  final DateTime occurredAt;

  const SecureHostActivity({
    required this.message,
    required this.peerName,
    required this.occurredAt,
  });
}

class SecureHostWorkspace extends StatefulWidget {
  final String localId;
  final List<SecureHostConnection> connections;
  final int? selectedConnectionId;
  final List<SecureHostActivity> activities;
  final Widget titleBar;
  final ValueChanged<int> onSelectConnection;
  final ValueChanged<int> onApprove;
  final ValueChanged<int> onReject;
  final ValueChanged<int> onDisconnect;
  final String? notice;
  final VoidCallback? onCopyLocalId;

  const SecureHostWorkspace({
    super.key,
    required this.localId,
    required this.connections,
    required this.selectedConnectionId,
    required this.activities,
    required this.titleBar,
    required this.onSelectConnection,
    required this.onApprove,
    required this.onReject,
    required this.onDisconnect,
    this.notice,
    this.onCopyLocalId,
  });

  @override
  State<SecureHostWorkspace> createState() => _SecureHostWorkspaceState();
}

class _Palette {
  static const green = Color(0xFF196B52);
  static const ink = Color(0xFF172C29);
  static const muted = Color(0xFF71807B);
  static const background = Color(0xFFF6F7F4);
  static const line = Color(0xFFE1E7E2);
  static const pale = Color(0xFFE7F1EB);
  static const danger = Color(0xFFA35740);
}

class _SecureHostWorkspaceState extends State<SecureHostWorkspace> {
  String _page = 'home';

  SecureHostConnection? get _selected {
    for (final connection in widget.connections) {
      if (connection.id == widget.selectedConnectionId) return connection;
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    return Material(
      color: _Palette.background,
      child: DefaultTextStyle(
        style: const TextStyle(fontSize: 13, color: _Palette.ink, height: 1.5),
        child: Column(
          children: [
            widget.titleBar,
            Expanded(
              child: LayoutBuilder(builder: (context, constraints) {
                final narrow = constraints.maxWidth < 1000;
                final short = constraints.maxHeight < 560;
                return Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _sidebar(narrow, short),
                    Expanded(
                      child: Padding(
                        padding: EdgeInsets.fromLTRB(narrow ? 20 : 30,
                            short ? 12 : 26, narrow ? 20 : 30, short ? 12 : 18),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            _heading(short),
                            SizedBox(height: short ? 12 : 20),
                            _statusBar(_selected, short),
                            SizedBox(height: short ? 12 : 18),
                            if (widget.connections.length > 1) ...[
                              _connectionSelector(),
                              const SizedBox(height: 12),
                            ],
                            Expanded(
                              child: SingleChildScrollView(
                                key: const ValueKey(
                                    'secure-host-content-scroll'),
                                child: _content(narrow),
                              ),
                            ),
                            _actionBar(_selected),
                            if (widget.notice != null &&
                                widget.notice!.isNotEmpty)
                              Padding(
                                padding: const EdgeInsets.only(top: 8),
                                child: Text(widget.notice!,
                                    maxLines: 2,
                                    overflow: TextOverflow.ellipsis,
                                    style: const TextStyle(
                                        fontSize: 11, color: _Palette.danger)),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ],
                );
              }),
            ),
          ],
        ),
      ),
    );
  }

  Widget _sidebar(bool narrow, bool short) {
    return Container(
      width: narrow ? 72 : 204,
      decoration: const BoxDecoration(
        color: Colors.white,
        border: Border(right: BorderSide(color: _Palette.line)),
      ),
      padding: EdgeInsets.fromLTRB(
          narrow ? 9 : 16, short ? 18 : 28, narrow ? 9 : 16, 18),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment:
                narrow ? MainAxisAlignment.center : MainAxisAlignment.start,
            children: [
              Container(
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                    color: _Palette.green,
                    borderRadius: BorderRadius.circular(11)),
                child: const Icon(Icons.desktop_windows_outlined,
                    color: Colors.white, size: 23),
              ),
              if (!narrow) ...[
                const SizedBox(width: 10),
                const Expanded(
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                      Text('Open Remote Desk',
                          style: TextStyle(
                              fontSize: 12, fontWeight: FontWeight.w600)),
                      Text('被控端',
                          style:
                              TextStyle(fontSize: 11, color: _Palette.muted)),
                    ])),
              ],
            ],
          ),
          SizedBox(height: short ? 24 : 40),
          if (!narrow)
            const Padding(
              padding: EdgeInsets.only(left: 14, bottom: 12),
              child: Text('工作空间',
                  style: TextStyle(
                      fontSize: 10, color: _Palette.muted, letterSpacing: 1)),
            ),
          _navigation('home', '本机访问', Icons.desktop_windows_outlined, narrow),
          const SizedBox(height: 7),
          _navigation('records', '访问记录', Icons.history_rounded, narrow),
          const SizedBox(height: 7),
          _navigation('settings', '安全设置', Icons.shield_outlined, narrow),
          const Spacer(),
          const Divider(color: _Palette.line, height: 28),
          Row(
              mainAxisAlignment:
                  narrow ? MainAxisAlignment.center : MainAxisAlignment.start,
              children: [
                const Icon(Icons.desktop_windows_outlined,
                    size: 20, color: _Palette.muted),
                if (!narrow) ...[
                  const SizedBox(width: 10),
                  const Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('当前电脑',
                            style: TextStyle(
                                fontSize: 12, fontWeight: FontWeight.w600)),
                        Text('Windows · 本机',
                            style:
                                TextStyle(fontSize: 10, color: _Palette.muted)),
                      ]),
                ],
              ]),
          if (!narrow && !short)
            const Padding(
              padding: EdgeInsets.only(top: 12),
              child: Text('Open Remote Desk · 现场批准',
                  style: TextStyle(fontSize: 9, color: _Palette.muted)),
            ),
        ],
      ),
    );
  }

  Widget _navigation(String page, String label, IconData icon, bool narrow) {
    final selected = _page == page;
    return Tooltip(
      message: label,
      child: TextButton(
        key: ValueKey('secure-host-nav-$page'),
        onPressed: () => setState(() => _page = page),
        style: TextButton.styleFrom(
          foregroundColor: selected ? _Palette.green : _Palette.muted,
          backgroundColor: selected ? _Palette.pale : Colors.transparent,
          minimumSize: const Size(0, 46),
          padding: EdgeInsets.symmetric(horizontal: narrow ? 0 : 14),
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
        ),
        child: Row(
            mainAxisAlignment:
                narrow ? MainAxisAlignment.center : MainAxisAlignment.start,
            children: [
              Icon(icon, size: 19),
              if (!narrow) ...[
                const SizedBox(width: 12),
                Expanded(
                    child: Text(label, style: const TextStyle(fontSize: 13))),
                if (selected) const Icon(Icons.circle, size: 5),
              ],
            ]),
      ),
    );
  }

  Widget _heading(bool short) {
    final title = _page == 'records'
        ? '访问记录'
        : _page == 'settings'
            ? '安全设置'
            : '本机远程访问';
    return Row(children: [
      Expanded(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        if (!short)
          const Text('你的电脑 · 你的决定',
              style: TextStyle(
                  fontSize: 10, color: _Palette.muted, letterSpacing: 2)),
        Text(title,
            style: TextStyle(
                fontSize: short ? 24 : 28, fontWeight: FontWeight.w600)),
      ])),
      Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        decoration: BoxDecoration(
            color: Colors.white,
            border: Border.all(color: _Palette.line),
            borderRadius: BorderRadius.circular(20)),
        child: const Row(mainAxisSize: MainAxisSize.min, children: [
          Icon(Icons.desktop_windows_outlined, size: 16, color: _Palette.muted),
          SizedBox(width: 7),
          Text('Windows 被控端',
              style: TextStyle(fontSize: 11, color: _Palette.muted)),
        ]),
      ),
    ]);
  }

  String _statusTitle(SecureHostConnection? connection) {
    switch (connection?.status) {
      case SecureHostStatus.pending:
        return '有新的连接请求';
      case SecureHostStatus.confirming:
        return '正在确认本次批准';
      case SecureHostStatus.active:
        return '连接已获本机批准';
      case SecureHostStatus.closing:
        return '正在结束本次连接';
      case SecureHostStatus.ended:
        return '本次连接已结束';
      case null:
        return '暂无连接请求';
    }
  }

  String _statusLabel(SecureHostConnection? connection) {
    switch (connection?.status) {
      case SecureHostStatus.pending:
        return '等待你的批准';
      case SecureHostStatus.confirming:
        return '正在确认';
      case SecureHostStatus.active:
        return '本次连接已批准';
      case SecureHostStatus.closing:
        return '正在结束';
      case SecureHostStatus.ended:
        return '本次授权已结束';
      case null:
        return '等待连接请求';
    }
  }

  String _statusDetail(SecureHostConnection? connection) {
    switch (connection?.status) {
      case SecureHostStatus.pending:
        return '等待本机决定，键鼠和文件能力未开放。';
      case SecureHostStatus.confirming:
        return '已提交本机决定，等待被控端确认。';
      case SecureHostStatus.active:
        return '本次连接已批准，屏幕共享还需本机启用视频并完成协商。';
      case SecureHostStatus.closing:
        return '正在等待被控端确认连接结束。';
      case SecureHostStatus.ended:
        return '本次连接授权已结束，重新连接需要重新批准。';
      case null:
        return '每次连接均需本机批准，视频需单独启用并协商。';
    }
  }

  Widget _statusBar(SecureHostConnection? connection, bool short) {
    final pending = connection?.status == SecureHostStatus.pending ||
        connection?.status == SecureHostStatus.confirming;
    final active = connection?.status == SecureHostStatus.active;
    return Container(
      padding: EdgeInsets.symmetric(horizontal: 15, vertical: short ? 10 : 14),
      decoration: BoxDecoration(
          color: const Color(0xFFEDF1EB),
          border: Border.all(color: _Palette.line),
          borderRadius: BorderRadius.circular(13)),
      child: Row(children: [
        Container(
            width: 40,
            height: 40,
            decoration: BoxDecoration(
                color: Colors.white, borderRadius: BorderRadius.circular(12)),
            child: Icon(pending ? Icons.person_outline : Icons.shield_outlined,
                color: _Palette.green, size: 21)),
        const SizedBox(width: 13),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(_statusTitle(connection),
              style:
                  const TextStyle(fontSize: 13, fontWeight: FontWeight.w600)),
          if (!short)
            Padding(
                padding: const EdgeInsets.only(top: 3),
                child: Text(_statusDetail(connection),
                    style:
                        const TextStyle(fontSize: 11, color: _Palette.muted))),
        ])),
        const SizedBox(width: 10),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
          decoration: BoxDecoration(
              color: pending
                  ? const Color(0xFFFFF4DA)
                  : active
                      ? const Color(0xFFDFF0E9)
                      : Colors.white,
              borderRadius: BorderRadius.circular(16)),
          child: Text(_statusLabel(connection),
              style: TextStyle(
                  fontSize: 10,
                  color: pending
                      ? const Color(0xFF9B6B22)
                      : active
                          ? _Palette.green
                          : _Palette.muted)),
        ),
      ]),
    );
  }

  Widget _connectionSelector() {
    return SizedBox(
      height: 42,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        itemCount: widget.connections.length,
        separatorBuilder: (_, __) => const SizedBox(width: 7),
        itemBuilder: (context, index) {
          final connection = widget.connections[index];
          final selected = connection.id == widget.selectedConnectionId;
          return OutlinedButton(
            key: ValueKey('secure-host-connection-${connection.id}'),
            onPressed: () => widget.onSelectConnection(connection.id),
            style: OutlinedButton.styleFrom(
              foregroundColor: selected ? _Palette.green : _Palette.muted,
              backgroundColor: selected ? _Palette.pale : Colors.white,
              side: BorderSide(
                  color: selected ? const Color(0xFF8DB6A4) : _Palette.line),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(9)),
            ),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 230),
              child: Text('${connection.name} · ${_statusLabel(connection)}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontSize: 11)),
            ),
          );
        },
      ),
    );
  }

  Widget _content(bool narrow) {
    if (_page == 'settings') {
      return _columns(
          narrow, [_securityCard(), _boundaries()], [_unavailableCard()]);
    }
    if (_page == 'records') {
      return _columns(narrow, [_activities(24)], [_boundaries()]);
    }
    return _columns(narrow, [_sessionCard(_selected), _activities(6)],
        [_permissions(), _boundaries()]);
  }

  Widget _columns(bool narrow, List<Widget> left, List<Widget> right) {
    Widget stack(List<Widget> children) =>
        Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          for (var i = 0; i < children.length; i++) ...[
            if (i > 0) const SizedBox(height: 18),
            children[i],
          ],
        ]);
    if (narrow) return stack([...left, ...right]);
    return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Expanded(flex: 128, child: stack(left)),
      const SizedBox(width: 18),
      Expanded(flex: 100, child: stack(right)),
    ]);
  }

  Widget _sessionCard(SecureHostConnection? connection) {
    if (connection == null) {
      return _HostCard(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const _Eyebrow(Icons.shield_outlined, '由你决定何时连接'),
        const _MonitorIllustration(),
        const Text('需要连接时，由你确认。',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 22, fontWeight: FontWeight.w600)),
        const SizedBox(height: 10),
        _body('收到连接请求后，请在这里确认。屏幕共享另需本机启用视频并完成协商。', centered: true),
        _localIdentity(),
        _note('当前支持 IP 直连与逐次现场批准。协助码与无人值守尚未实现。'),
      ]));
    }
    final active = connection.status == SecureHostStatus.active;
    final ended = connection.status == SecureHostStatus.ended;
    final confirming = connection.status == SecureHostStatus.confirming;
    final closing = connection.status == SecureHostStatus.closing;
    final title = ended
        ? '连接结束，安心继续。'
        : active
            ? '本次会话已获批准'
            : confirming
                ? '正在确认你的批准'
                : closing
                    ? '正在收回本次许可'
                    : '有人想连接这台电脑';
    return _HostCard(
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      _Eyebrow(
          active ? Icons.desktop_windows_outlined : Icons.shield_outlined,
          ended
              ? '本次连接已经结束'
              : active
                  ? '当前连接'
                  : '每次连接，由你确认'),
      if (ended) const _MonitorIllustration(),
      const SizedBox(height: 14),
      Text(title,
          style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w600)),
      const SizedBox(height: 9),
      _body(connection.detail ??
          (ended
              ? '本次连接授权已结束，再次连接需要重新批准。'
              : active
                  ? '连接已获本机批准，键鼠与文件能力仍未开放。屏幕共享还需本机启用视频并完成本次协商。'
                  : confirming
                      ? '已提交本机决定，等待被控端确认。本次请求不会获得键鼠或文件权限。'
                      : closing
                          ? '结束请求已提交，请等待连接关闭。'
                          : '请确认这是预期的连接请求，再允许对方连接。')),
      Padding(
        padding: const EdgeInsets.symmetric(vertical: 19),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Container(
              width: 44,
              height: 44,
              decoration: BoxDecoration(
                  color: const Color(0xFFECF2E9),
                  borderRadius: BorderRadius.circular(13)),
              child: const Icon(Icons.person_outline,
                  color: _Palette.green, size: 24)),
          const SizedBox(width: 11),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(connection.name,
                    style: const TextStyle(
                        fontSize: 14, fontWeight: FontWeight.w600)),
                const SizedBox(height: 5),
                Text('对方自报 ID：${connection.peerId}',
                    style:
                        const TextStyle(fontSize: 11, color: _Palette.muted)),
                const Text('名称与 ID 由对方提供，请核对来意',
                    style: TextStyle(fontSize: 10, color: _Palette.muted)),
              ])),
        ]),
      ),
      if (active)
        Container(
          decoration: BoxDecoration(
              color: const Color(0xFFF5F8F1),
              border: Border.all(color: _Palette.line),
              borderRadius: BorderRadius.circular(12)),
          child: const Column(children: [
            Padding(
                padding: EdgeInsets.fromLTRB(14, 11, 14, 0),
                child: Align(
                    alignment: Alignment.centerLeft,
                    child: Text('主显示器 · 共享范围说明',
                        style:
                            TextStyle(fontSize: 11, color: _Palette.muted)))),
            _MonitorIllustration(),
            Text('视频启用并协商后，才会共享主显示器',
                style: TextStyle(fontSize: 12, color: _Palette.green)),
            Padding(
                padding: EdgeInsets.fromLTRB(12, 5, 12, 15),
                child: Text('静态范围示意，不提供实时画面反馈',
                    style: TextStyle(fontSize: 10, color: _Palette.muted))),
          ]),
        )
      else if (!ended)
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
              color: const Color(0xFFF2F7ED),
              border: Border.all(color: const Color(0xFFDDE8D7)),
              borderRadius: BorderRadius.circular(11)),
          child: const Row(children: [
            Icon(Icons.desktop_windows_outlined,
                color: _Palette.green, size: 19),
            SizedBox(width: 8),
            Expanded(child: Text('请求连接本机', style: TextStyle(fontSize: 12))),
            Text('现场批准', style: TextStyle(fontSize: 10, color: _Palette.green))
          ]),
        ),
      if (!ended)
        const Padding(
            padding: EdgeInsets.only(top: 17),
            child: Row(children: [
              Icon(Icons.lock_outline, size: 15, color: _Palette.muted),
              SizedBox(width: 7),
              Text('IP 直连 · v1 加密通道',
                  style: TextStyle(fontSize: 11, color: _Palette.muted))
            ])),
      if (active && connection.elapsedText != null)
        Padding(
            padding: const EdgeInsets.only(top: 9),
            child: _body('本机批准后已连接 ${connection.elapsedText}')),
      if (!active) _note('此次批准仅适用于当前连接，不登记长期信任。'),
    ]));
  }

  Widget _localIdentity() {
    final available = widget.localId.isNotEmpty;
    return Container(
      margin: const EdgeInsets.only(top: 20),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
          color: const Color(0xFFF8FAF6),
          border: Border.all(color: const Color(0xFFE4EBE0)),
          borderRadius: BorderRadius.circular(12)),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const Text('本机设备 ID',
            style: TextStyle(fontSize: 11, color: _Palette.muted)),
        const SizedBox(height: 8),
        Row(children: [
          Expanded(
              child: SelectableText(available ? widget.localId : '暂不可用',
                  style: TextStyle(
                      fontSize: available ? 22 : 13,
                      color: _Palette.ink,
                      letterSpacing: available ? 1 : 0))),
          const SizedBox(width: 8),
          OutlinedButton(
              key: const ValueKey('secure-host-copy-id'),
              onPressed: available ? widget.onCopyLocalId : null,
              style: OutlinedButton.styleFrom(
                  foregroundColor: _Palette.green,
                  side: const BorderSide(color: _Palette.line),
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8))),
              child: const Text('复制 ID', style: TextStyle(fontSize: 11))),
        ]),
        const SizedBox(height: 9),
        const Text('连接还需可信设备公钥与 IP 地址。请通过可信方式核对设备资料。',
            style: TextStyle(fontSize: 10, color: _Palette.muted, height: 1.8)),
      ]),
    );
  }

  Widget _permissions() {
    return _HostCard(
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      const _CardHeading('本次会话权限', Icons.shield_outlined),
      const SizedBox(height: 8),
      _body('连接批准不会授予键鼠或文件权限。'),
      _permission(Icons.desktop_windows_outlined, '查看屏幕', '需本机启用视频，并完成本次视频协商',
          '需视频启用并协商', false),
      _permission(Icons.mouse_outlined, '键盘与鼠标', '电脑操作仍由本机掌控', '尚未实现', false),
      _permission(Icons.folder_outlined, '上传到本机', '文件权限不随查看开放', '尚未实现', false),
      _permission(Icons.folder_outlined, '从本机下载', '未开放文件与目录浏览', '尚未实现', false),
      _note('剪贴板、音频、终端、远程重启和管理员提权均未开放。'),
    ]));
  }

  Widget _permission(
      IconData icon, String title, String detail, String status, bool active) {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 17),
      decoration: const BoxDecoration(
          border: Border(bottom: BorderSide(color: Color(0xFFEEF1EA)))),
      child: Row(children: [
        Container(
            width: 34,
            height: 34,
            decoration: BoxDecoration(
                color: const Color(0xFFF5F7F2),
                borderRadius: BorderRadius.circular(10)),
            child: Icon(icon, size: 19, color: _Palette.muted)),
        const SizedBox(width: 10),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              style:
                  const TextStyle(fontSize: 12, fontWeight: FontWeight.w600)),
          const SizedBox(height: 4),
          Text(detail,
              style: const TextStyle(fontSize: 10, color: _Palette.muted)),
        ])),
        const SizedBox(width: 8),
        Text(status,
            style: TextStyle(
                fontSize: 10, color: active ? _Palette.green : _Palette.muted)),
      ]),
    );
  }

  Widget _boundaries() {
    return _HostCard(
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      const _CardHeading('连接方式，各有边界', Icons.lock_outline),
      _boundary('每次连接，现场批准', '由本机决定是否允许当前连接。'),
      _boundary('只读能力，单独启用', '屏幕共享还需启用视频并完成协商，键鼠与文件未开放。'),
      _boundary('结束即收回本次许可', '断开后旧授权失效，再次连接需要重新批准。'),
    ]));
  }

  Widget _boundary(String title, String detail) {
    return Padding(
      padding: const EdgeInsets.only(top: 18),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Container(
            width: 21,
            height: 21,
            decoration: const BoxDecoration(
                color: Color(0xFFEDF4E6), shape: BoxShape.circle),
            child: const Icon(Icons.check, size: 14, color: Color(0xFF729565))),
        const SizedBox(width: 10),
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              style:
                  const TextStyle(fontSize: 12, fontWeight: FontWeight.w500)),
          const SizedBox(height: 4),
          _body(detail)
        ])),
      ]),
    );
  }

  Widget _activities(int limit) {
    final entries = widget.activities.reversed.take(limit).toList();
    return _HostCard(
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      _CardHeading(limit == 6 ? '本次活动' : '本次运行的访问记录', Icons.history_rounded),
      if (entries.isEmpty)
        Padding(
            padding: const EdgeInsets.symmetric(vertical: 26),
            child: _body('还没有连接或批准事件。\n收到真实请求后，会在这里显示。', centered: true)),
      for (final entry in entries)
        Container(
          padding: const EdgeInsets.symmetric(vertical: 12),
          decoration: const BoxDecoration(
              border: Border(bottom: BorderSide(color: Color(0xFFEEF1EA)))),
          child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            const Padding(
                padding: EdgeInsets.only(top: 6),
                child: Icon(Icons.circle, size: 5, color: Color(0xFFAAC39C))),
            const SizedBox(width: 10),
            Expanded(
                child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                  Text(entry.message, style: const TextStyle(fontSize: 12)),
                  Text(entry.peerName,
                      style:
                          const TextStyle(fontSize: 10, color: _Palette.muted))
                ])),
            const SizedBox(width: 8),
            Text(
                '${entry.occurredAt.hour.toString().padLeft(2, '0')}:${entry.occurredAt.minute.toString().padLeft(2, '0')}',
                style: const TextStyle(fontSize: 10, color: _Palette.muted)),
          ]),
        ),
      const SizedBox(height: 10),
      const Text('仅显示当前运行最近 24 条事件。不记录画面、输入文字或访问凭据。',
          style: TextStyle(fontSize: 10, color: _Palette.muted, height: 1.8)),
    ]));
  }

  Widget _securityCard() {
    return _HostCard(
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      const _Eyebrow(Icons.shield_outlined, '已实现的访问边界'),
      const SizedBox(height: 15),
      const Text('你的电脑，你的决定。',
          style: TextStyle(fontSize: 22, fontWeight: FontWeight.w600)),
      const SizedBox(height: 9),
      _body('本轮使用严格身份验证、加密通道和逐次现场批准，电脑操作仍由本机掌控。'),
      _setting('连接方式', 'IP 直连'),
      _setting('通信保护', 'v1 加密通道'),
      _setting('连接许可', '每次由本机批准'),
      _setting('重连策略', '重新批准'),
      _localIdentity(),
      _note('本页面展示访问边界，不改变系统服务或远程访问的启动配置。'),
    ]));
  }

  Widget _setting(String label, String value) {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 18),
      decoration: const BoxDecoration(
          border: Border(bottom: BorderSide(color: Color(0xFFEEF1EA)))),
      child: Row(children: [
        Expanded(child: Text(label, style: const TextStyle(fontSize: 12))),
        const SizedBox(width: 15),
        Text(value, style: const TextStyle(fontSize: 11, color: _Palette.muted))
      ]),
    );
  }

  Widget _unavailableCard() {
    const capabilities = {
      'unattended': '无人值守',
      'trusted-devices': '可信控制设备登记',
      'invite': '临时协助码',
      'keyboard': '键盘与鼠标控制',
      'files': '文件上传与下载',
      'clipboard': '剪贴板与音频',
      'elevation': '终端与管理员提权',
    };
    return _HostCard(
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      const _CardHeading('尚未开放的能力', Icons.lock_outline),
      const SizedBox(height: 9),
      _body('以下功能仍未实现，不能在此开启。'),
      const SizedBox(height: 18),
      for (final entry in capabilities.entries)
        Padding(
          padding: const EdgeInsets.only(bottom: 9),
          child: OutlinedButton(
            key: ValueKey('secure-host-capability-${entry.key}'),
            onPressed: null,
            style: OutlinedButton.styleFrom(
              minimumSize: const Size(0, 44),
              disabledForegroundColor: _Palette.muted,
              backgroundColor: const Color(0xFFF6F8F3),
              side: const BorderSide(color: Color(0xFFE6ECE0)),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8)),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
            ),
            child: Row(children: [
              Expanded(
                  child:
                      Text(entry.value, style: const TextStyle(fontSize: 12))),
              const SizedBox(width: 8),
              const Text('尚未实现', style: TextStyle(fontSize: 10))
            ]),
          ),
        ),
    ]));
  }

  Widget _actionBar(SecureHostConnection? connection) {
    if (connection == null || connection.status == SecureHostStatus.ended) {
      return Container(
        padding: const EdgeInsets.only(top: 14),
        margin: const EdgeInsets.only(top: 12),
        decoration: const BoxDecoration(
            border: Border(top: BorderSide(color: _Palette.line))),
        child: const Text('批准只对本次连接有效 · 视频另需启用并协商',
            style: TextStyle(fontSize: 11, color: _Palette.muted)),
      );
    }
    final confirming = connection.status == SecureHostStatus.confirming;
    final closing = connection.status == SecureHostStatus.closing;
    final active = connection.status == SecureHostStatus.active;
    final actions = <Widget>[
      if (active || closing)
        OutlinedButton(
          key: const ValueKey('secure-host-disconnect'),
          onPressed: closing ? null : () => widget.onDisconnect(connection.id),
          style: OutlinedButton.styleFrom(
              foregroundColor: _Palette.danger,
              backgroundColor: const Color(0xFFFFF8F4),
              minimumSize: const Size(0, 44),
              side: const BorderSide(color: Color(0xFFEFDDD4)),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(9))),
          child: Text(closing ? '正在结束…' : '结束本次连接'),
        )
      else ...[
        OutlinedButton(
          key: const ValueKey('secure-host-reject'),
          onPressed: () => widget.onReject(connection.id),
          style: OutlinedButton.styleFrom(
              foregroundColor: _Palette.muted,
              backgroundColor: Colors.white,
              minimumSize: const Size(0, 44),
              side: const BorderSide(color: _Palette.line),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(9))),
          child: Text(confirming ? '取消本次请求' : '拒绝'),
        ),
        const SizedBox(width: 10),
        FilledButton(
          key: const ValueKey('secure-host-approve'),
          onPressed: confirming ? null : () => widget.onApprove(connection.id),
          style: FilledButton.styleFrom(
              backgroundColor: _Palette.green,
              foregroundColor: Colors.white,
              minimumSize: const Size(0, 44),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(9))),
          child: Text(confirming ? '正在确认…' : '批准连接'),
        ),
      ],
    ];
    return Container(
      key: const ValueKey('secure-host-action-bar'),
      padding: const EdgeInsets.only(top: 14),
      margin: const EdgeInsets.only(top: 14),
      decoration: const BoxDecoration(
          border: Border(top: BorderSide(color: _Palette.line))),
      child: Row(children: [
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(connection.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style:
                  const TextStyle(fontSize: 12, fontWeight: FontWeight.w600)),
          Text(
              active
                  ? '连接已批准 · 键鼠与文件未开放'
                  : confirming
                      ? '等待被控端确认批准'
                      : closing
                          ? '正在等待连接结束'
                          : '当前请求 · 尚未批准连接',
              style: const TextStyle(fontSize: 10, color: _Palette.muted)),
        ])),
        const SizedBox(width: 18),
        ...actions,
      ]),
    );
  }

  Widget _body(String text, {bool centered = false}) => Text(text,
      textAlign: centered ? TextAlign.center : TextAlign.start,
      style: const TextStyle(fontSize: 12, color: _Palette.muted, height: 1.8));

  Widget _note(String text) => Container(
      margin: const EdgeInsets.only(top: 16),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
          color: const Color(0xFFF5F7F0),
          borderRadius: BorderRadius.circular(10)),
      child: Text(text,
          style: const TextStyle(
              fontSize: 11, color: _Palette.muted, height: 1.8)));
}

class _HostCard extends StatelessWidget {
  final Widget child;
  const _HostCard({required this.child});

  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.all(23),
        decoration: BoxDecoration(
            color: Colors.white,
            border: Border.all(color: _Palette.line),
            borderRadius: BorderRadius.circular(18)),
        child: child,
      );
}

class _CardHeading extends StatelessWidget {
  final String text;
  final IconData icon;
  const _CardHeading(this.text, this.icon);

  @override
  Widget build(BuildContext context) => Row(children: [
        Expanded(
            child: Text(text,
                style: const TextStyle(
                    fontSize: 14, fontWeight: FontWeight.w600))),
        const SizedBox(width: 8),
        Icon(icon, size: 19, color: _Palette.muted),
      ]);
}

class _Eyebrow extends StatelessWidget {
  final IconData icon;
  final String text;
  const _Eyebrow(this.icon, this.text);

  @override
  Widget build(BuildContext context) => Row(children: [
        Icon(icon, size: 16, color: _Palette.green),
        const SizedBox(width: 7),
        Expanded(
            child: Text(text,
                style: const TextStyle(
                    fontSize: 11,
                    color: _Palette.green,
                    fontWeight: FontWeight.w600))),
      ]);
}

class _MonitorIllustration extends StatelessWidget {
  const _MonitorIllustration();

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 20),
        child: Column(children: [
          Container(
            width: 162,
            height: 100,
            decoration: BoxDecoration(
                color: const Color(0xFFF2F7F1),
                border: Border.all(color: const Color(0xFFB9CFC2), width: 3),
                borderRadius: BorderRadius.circular(11)),
            child: Column(children: [
              Container(
                  height: 14,
                  padding: const EdgeInsets.only(left: 8),
                  alignment: Alignment.centerLeft,
                  decoration: const BoxDecoration(
                      border:
                          Border(bottom: BorderSide(color: Color(0xFFE0EADD)))),
                  child: const Text('···',
                      style: TextStyle(color: Color(0xFFBDCDBD), height: 1))),
              const SizedBox(height: 10),
              const Icon(Icons.shield_outlined,
                  size: 34, color: Color(0xFF6D947D)),
              const SizedBox(height: 7),
              Container(height: 3, width: 40, color: const Color(0xFFD4E3D2)),
            ]),
          ),
          Container(width: 10, height: 13, color: const Color(0xFFBED2C5)),
          Container(
              width: 66,
              height: 4,
              decoration: BoxDecoration(
                  color: const Color(0xFFB9CFC2),
                  borderRadius: BorderRadius.circular(3))),
        ]),
      );
}
