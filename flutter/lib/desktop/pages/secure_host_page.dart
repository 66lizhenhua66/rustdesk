import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

import '../../common.dart';
import '../../models/platform_model.dart';
import '../../models/server_model.dart';
import '../widgets/secure_host_workspace.dart';

class SecureHostPage extends StatefulWidget {
  const SecureHostPage({super.key});

  @override
  State<SecureHostPage> createState() => _SecureHostPageState();
}

class _SecureHostPageState extends State<SecureHostPage> with WindowListener {
  late final ServerModel _model;
  final Set<int> _approving = {};
  final Set<int> _closing = {};
  final List<SecureHostActivity> _activities = [];
  List<SecureHostConnection> _connections = [];
  SecureHostConnection? _lastEnded;
  int? _selectedId;
  String _localId = '';
  String? _notice;
  bool _closingWindow = false;

  @override
  void initState() {
    super.initState();
    _model = gFFI.serverModel;
    Get.put(_model.tabController);
    gFFI.ffiModel.updateEventListener(gFFI.sessionId, '');
    _model.addListener(_onModelChanged);
    windowManager.addListener(this);
    _onModelChanged();
    _loadIdentity();
  }

  @override
  void dispose() {
    _model.removeListener(_onModelChanged);
    windowManager.removeListener(this);
    super.dispose();
  }

  Future<void> _loadIdentity() async {
    try {
      final id = await bind.cmGetConfig(name: 'id');
      if (mounted) setState(() => _localId = id);
    } catch (_) {
      if (mounted) {
        setState(() => _notice = '本机设备 ID 暂不可用，请稍后重新打开工作台。');
      }
    }
  }

  Client? _liveClient(int id) {
    for (final client in _model.clients) {
      if (client.id == id && !client.disconnected) return client;
    }
    return null;
  }

  SecureHostStatus _status(Client client) {
    if (client.disconnected) return SecureHostStatus.ended;
    if (_closing.contains(client.id)) return SecureHostStatus.closing;
    if (client.authorized) return SecureHostStatus.active;
    if (_approving.contains(client.id)) return SecureHostStatus.confirming;
    return SecureHostStatus.pending;
  }

  void _record(String message, String name) {
    _activities.add(SecureHostActivity(
      message: message,
      peerName: name,
      occurredAt: DateTime.now(),
    ));
    if (_activities.length > 24) _activities.removeAt(0);
  }

  void _onModelChanged() {
    if (!mounted) return;
    final previous = {for (final item in _connections) item.id: item};
    final next = <SecureHostConnection>[];
    for (final client in _model.clients) {
      final status = _status(client);
      final item = SecureHostConnection(
        id: client.id,
        name: client.name.isEmpty ? '未提供名称的控制端' : client.name,
        peerId: client.peerId,
        status: status,
        inputSupported: client.ordInputSupported,
        inputEnabled:
            client.authorized && client.keyboard && !client.disconnected,
      );
      next.add(item);
      final old = previous[client.id];
      if (old != null &&
          old.inputEnabled != item.inputEnabled &&
          !client.ordInputReleaseFailed) {
        _record(item.inputEnabled ? '被控端已允许本次键鼠操作' : '被控端已撤销键鼠操作', item.name);
        _notice = null;
      }
      if (client.ordInputReleaseFailed) {
        _notice = '${item.name} 的输入许可已失效，但按键释放失败。请在本机检查按键状态，并重启被控服务后重试。';
      }
      if (old == null) {
        _selectedId = client.id;
        _record(status == SecureHostStatus.active ? '被控端已确认本次连接批准' : '收到新的连接请求',
            item.name);
      } else if (old.status != status) {
        if (status == SecureHostStatus.active) {
          _record('被控端已确认本次连接批准', item.name);
        } else if (status == SecureHostStatus.ended) {
          _record('本次连接已结束，本次许可已失效', item.name);
        }
      }
      if (client.authorized || client.disconnected) {
        _approving.remove(client.id);
      }
      if (client.disconnected) _closing.remove(client.id);
    }
    final currentIds = next.map((item) => item.id).toSet();
    for (final old in _connections) {
      if (currentIds.contains(old.id)) continue;
      if (old.status != SecureHostStatus.ended) {
        _record('本次连接已结束，本次许可已失效', old.name);
      }
      _lastEnded = SecureHostConnection(
        id: old.id,
        name: old.name,
        peerId: old.peerId,
        status: SecureHostStatus.ended,
      );
      _approving.remove(old.id);
      _closing.remove(old.id);
    }
    if (next.isEmpty && _lastEnded != null) next.add(_lastEnded!);
    if (!next.any((item) => item.id == _selectedId)) {
      _selectedId = next.isEmpty ? null : next.last.id;
    }
    setState(() => _connections = next);
  }

  void _approve(int id) {
    final client = _liveClient(id);
    if (client == null ||
        client.authorized ||
        _approving.contains(id) ||
        _closing.contains(id)) return;
    _withLocalDecision(id, () async {
      final current = _liveClient(id);
      if (!mounted ||
          current == null ||
          current.authorized ||
          _approving.contains(id) ||
          _closing.contains(id)) return;
      _approving.add(id);
      _notice = null;
      _record('已提交本机批准，等待确认', current.name);
      _onModelChanged();
      try {
        await bind.cmLoginRes(connId: id, res: true);
      } catch (_) {
        if (!mounted) return;
        _approving.remove(id);
        _notice = '本机批准未能提交，请重试。';
        _onModelChanged();
      }
    });
  }

  void _end(int id) async {
    final current = _liveClient(id);
    if (!mounted || current == null || _closing.contains(id)) return;
    // Ending access must remain available even while remote input is arriving.
    _closing.add(id);
    _notice = null;
    _record(current.authorized ? '已请求结束本次连接' : '已提交拒绝请求', current.name);
    _onModelChanged();
    try {
      await bind.cmCloseConnection(connId: id);
    } catch (_) {
      if (!mounted) return;
      _closing.remove(id);
      _notice = '结束请求未能提交，请重试。';
      _onModelChanged();
    }
  }

  Future<void> _copyIdentity() async {
    try {
      await Clipboard.setData(ClipboardData(text: _localId));
      if (mounted) setState(() => _notice = '本机设备 ID 已复制。');
    } catch (_) {
      if (mounted) setState(() => _notice = '复制失败，请选中设备 ID 后复制。');
    }
  }

  void _setInputPermission(int id, bool enabled) {
    Future<void> submit() async {
      final client = _liveClient(id);
      if (!mounted ||
          client == null ||
          !client.authorized ||
          (enabled && !client.ordInputSupported) ||
          _closing.contains(id)) return;
      setState(() => _notice = enabled
          ? '已提交允许键鼠请求；会话权限将显示本机确认结果。同一时刻仅一个连接可操作。'
          : '已提交撤销请求，正在等待本机清理按键并确认。');
      try {
        await bind.cmSwitchPermission(
            connId: id, name: 'keyboard', enabled: enabled);
      } catch (_) {
        if (mounted) setState(() => _notice = '键鼠权限请求未能提交，请重试。');
      }
    }

    if (enabled) {
      _withLocalDecision(id, submit);
    } else {
      submit();
    }
  }

  Future<void> _withLocalDecision(
      int id, Future<void> Function() action) async {
    // Strict approval never uses the legacy allow-remote-CM-modification option.
    final clickedAt = DateTime.now().millisecondsSinceEpoch;
    try {
      await bind.cmCheckClickTime(connId: id);
      await Future<void>.delayed(const Duration(milliseconds: 120));
      final lastRemoteInput = await bind.cmGetClickTime();
      if (!mounted) return;
      if (clickedAt - lastRemoteInput > 120) {
        await action();
      } else {
        setState(() => _notice = '刚检测到远端输入，请先停止远端操作，再在本机确认。撤销与结束始终可用。');
      }
    } catch (_) {
      if (mounted) setState(() => _notice = '无法确认本机操作，请重试。');
    }
  }

  @override
  void onWindowClose() async {
    if (_closingWindow) return;
    _closingWindow = true;
    try {
      await _model.closeAll();
      await gFFI.close();
      await windowManager.setPreventClose(false);
      await windowManager.close();
    } catch (_) {
      _closingWindow = false;
      if (mounted) setState(() => _notice = '窗口尚未关闭，请先结束当前连接后重试。');
    }
  }

  Widget _titleBar() => Container(
        height: 36,
        color: Colors.white,
        child: Row(children: [
          Expanded(
              child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onPanStart: (_) => windowManager.startDragging(),
            child: const Padding(
              padding: EdgeInsets.symmetric(horizontal: 14),
              child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    'Open Remote Desk · 被控端',
                    style: TextStyle(fontSize: 12, color: Color(0xFF71807B)),
                  )),
            ),
          )),
          IconButton(
              tooltip: '最小化',
              iconSize: 18,
              onPressed: () => windowManager.minimize(),
              icon: const Icon(Icons.remove)),
          IconButton(
              tooltip: '最大化或还原',
              iconSize: 16,
              onPressed: () async {
                if (await windowManager.isMaximized()) {
                  await windowManager.unmaximize();
                } else {
                  await windowManager.maximize();
                }
              },
              icon: const Icon(Icons.crop_square)),
          IconButton(
              tooltip: '关闭窗口并结束当前会话',
              iconSize: 18,
              onPressed: () => windowManager.close(),
              icon: const Icon(Icons.close)),
        ]),
      );

  @override
  Widget build(BuildContext context) => SecureHostWorkspace(
        localId: _localId,
        connections: _connections,
        selectedConnectionId: _selectedId,
        activities: List.unmodifiable(_activities),
        notice: _notice,
        titleBar: _titleBar(),
        onSelectConnection: (id) => setState(() => _selectedId = id),
        onApprove: _approve,
        onReject: _end,
        onDisconnect: _end,
        onInputPermission: _setInputPermission,
        onCopyLocalId: _localId.isEmpty ? null : _copyIdentity,
      );
}
