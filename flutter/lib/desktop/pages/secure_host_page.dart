import 'dart:convert';

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
  bool _accessLoading = false;
  bool _accessChanging = false;
  bool _unattendedEnabled = false;
  List<SecureTrustedDevice> _trustedDevices = [];

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
    _loadTrusted();
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
        pairPending: client.ordAccessPairPending,
        unattended: client.ordAccessUnattended,
        detail: client.ordAccessUnattended
            ? '已验证登记设备的只读凭据。可结束当前查看，或在安全设置撤销此设备。'
            : client.ordAccessPairPending
                ? '请核对请求中的设备指纹；登记后允许该设备在有效期内无人值守查看。'
                : null,
      );
      next.add(item);
      final old = previous[client.id];
      if (old != null &&
          old.inputEnabled != item.inputEnabled &&
          !client.ordInputReleaseFailed) {
        _record(item.inputEnabled ? '控制端已开启键鼠' : '键鼠已关闭', item.name);
        _notice = null;
      }
      if (client.ordInputReleaseFailed) {
        _notice = '${item.name} 的输入许可已失效，但按键释放失败。请在本机检查按键状态，并重启被控服务后重试。';
      }
      if (old == null) {
        _selectedId = client.id;
        _record(
            status == SecureHostStatus.active
                ? (item.unattended ? '可信设备凭据已验证' : '被控端已确认本次连接批准')
                : (item.pairPending ? '收到只读访问登记请求' : '收到新的连接请求'),
            item.name);
      } else if (old.status != status) {
        if (status == SecureHostStatus.active) {
          _record(item.unattended ? '可信设备凭据已验证' : '被控端已确认本次连接批准', item.name);
          _loadTrusted();
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
        client.ordAccessPairPending ||
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

  Future<void> _loadTrusted() async {
    if (_accessLoading || !mounted) return;
    _accessLoading = true;
    try {
      final raw = await bind.cmSecureAccess(command: 'list', value: '');
      final result = jsonDecode(raw) as Map<String, dynamic>;
      if (result['ok'] != true) { throw StateError('可信设备状态不可用'); }
      final devices = (result['devices'] as List<dynamic>).map((raw) {
        final device = raw as Map<String, dynamic>;
        return SecureTrustedDevice(
            id: device['id'] as String,
            name: device['name'] as String,
            fingerprint: device['fingerprint'] as String,
            expiresAt: (device['expiresAt'] as num).toInt());
      }).toList();
      if (mounted) {
        setState(() {
          _trustedDevices = devices;
          _unattendedEnabled = result['enabled'] == true;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _notice = '可信设备列表无法读取，请刷新或检查本机授权存储。');
    } finally {
      _accessLoading = false;
    }
  }

  void _pair(int id) {
    final client = _liveClient(id);
    if (client == null ||
        client.authorized ||
        !client.ordAccessPairPending ||
        _approving.contains(id) ||
        _closing.contains(id)) return;
    _withLocalDecision(id, () async {
      final current = _liveClient(id);
      if (!mounted ||
          current == null ||
          current.authorized ||
          !current.ordAccessPairPending) return;
      _approving.add(id);
      _record('已提交只读访问登记，等待确认', current.name);
      _onModelChanged();
      try {
        if (!await bind.cmPairSecureAccess(connId: id)) {
          throw StateError('登记请求未提交');
        }
      } catch (_) {
        if (!mounted) return;
        _approving.remove(id);
        _notice = '设备登记未能提交，请核对当前请求后重试。';
        _onModelChanged();
      }
    });
  }

  Future<void> _revokeTrusted(String? id) async {
    if (_accessChanging || _accessLoading) return;
    setState(() => _accessChanging = true);
    try {
      final raw = await bind.cmSecureAccess(
          command: id == null ? 'revoke_all' : 'revoke', value: id ?? '');
      final result = jsonDecode(raw) as Map<String, dynamic>;
      if (result['ok'] != true) throw StateError('撤销未写入');
      if (!mounted) return;
      setState(() =>
          _notice = id == null ? '已撤销全部设备，相关会话即将结束。' : '已撤销此设备，相关会话即将结束。');
      await _loadTrusted();
    } catch (_) {
      if (mounted) setState(() => _notice = '撤销未能保存，请重试；必要时先结束相关连接。');
    } finally {
      if (mounted) setState(() => _accessChanging = false);
    }
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
        setState(() => _notice = '刚检测到远端输入，请先停止远端操作，再在本机确认。结束连接始终可用。');
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
        onCopyLocalId: _localId.isEmpty ? null : _copyIdentity,
        onPair: _pair,
        trustedDevices: _trustedDevices,
        unattendedEnabled: _unattendedEnabled,
        onRefreshTrusted: _accessLoading || _accessChanging ? null : _loadTrusted,
        onRevokeTrusted: _accessChanging || _accessLoading ? null : _revokeTrusted,
        onRevokeAllTrusted: _accessChanging || _accessLoading ? null : () => _revokeTrusted(null),
      );
}
