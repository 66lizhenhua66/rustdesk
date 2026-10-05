import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/secure_host_workspace.dart';
import 'package:flutter_test/flutter_test.dart';

SecureHostConnection connection(int id, SecureHostStatus status) {
  return SecureHostConnection(
    id: id,
    name: '控制设备 $id',
    peerId: 'peer-$id',
    status: status,
  );
}

Widget workspace({
  required List<SecureHostConnection> connections,
  int? selectedId,
  ValueChanged<int>? onApprove,
  ValueChanged<int>? onReject,
  ValueChanged<int>? onDisconnect,
  ValueChanged<int>? onSelect,
  ValueChanged<int>? onPair,
  List<SecureTrustedDevice> trustedDevices = const [],
  ValueChanged<String>? onRevoke,
}) {
  return MaterialApp(
    home: SecureHostWorkspace(
      localId: '123456789',
      connections: connections,
      selectedConnectionId: selectedId,
      activities: const [],
      titleBar: const SizedBox(height: 32),
      onSelectConnection: onSelect ?? (_) {},
      onApprove: onApprove ?? (_) {},
      onReject: onReject ?? (_) {},
      onDisconnect: onDisconnect ?? (_) {},
      onPair: onPair,
      trustedDevices: trustedDevices,
      onRevokeTrusted: onRevoke,
    ),
  );
}

Future<void> setWindow(WidgetTester tester, Size size) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  await tester.pump();
}

void main() {
  testWidgets(
      'pairing requires its explicit action instead of ordinary approval',
      (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await setWindow(tester, const Size(840, 620));
    final pairs = <int>[];
    final approvals = <int>[];
    await tester.pumpWidget(workspace(connections: [
      const SecureHostConnection(
          id: 12,
          name: '登记设备',
          peerId: 'peer',
          status: SecureHostStatus.pending,
          pairPending: true)
    ], selectedId: 12, onApprove: approvals.add, onPair: pairs.add));
    expect(find.text('批准连接'), findsNothing);
    expect(find.byKey(const ValueKey('secure-host-pair')).hitTestable(),
        findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('secure-host-pair')));
    expect(pairs, [12]);
    expect(approvals, isEmpty);
    expect(tester.takeException(), isNull);
  });

  testWidgets('trusted-device revoke targets the selected stored credential',
      (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await setWindow(tester, const Size(1120, 760));
    final revoked = <String>[];
    await tester.pumpWidget(workspace(connections: [], trustedDevices: const [
      SecureTrustedDevice(
          id: 'grant-1',
          name: '我的手机',
          fingerprint: '1234abcd',
          expiresAt: 1792000000000)
    ], onRevoke: revoked.add));
    await tester.tap(find.text('安全设置'));
    await tester.pumpAndSettle();
    final target = find.byKey(const ValueKey('secure-host-revoke-grant-1'));
    await tester.scrollUntilVisible(target, 250,
        scrollable: find.byType(Scrollable).first);
    await tester.tap(target);
    expect(revoked, ['grant-1']);
    expect(tester.takeException(), isNull);
  });
  testWidgets('host approves access once and reports controller input state',
      (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await setWindow(tester, const Size(840, 620));
    final disconnected = <int>[];
    Widget current(SecureHostStatus status, bool supported, bool enabled) =>
        workspace(
          connections: [
            SecureHostConnection(
                id: 9,
                name: '控制端',
                peerId: 'peer',
                status: status,
                inputSupported: supported,
                inputEnabled: enabled)
          ],
          selectedId: 9,
          onDisconnect: disconnected.add,
        );
    await tester.pumpWidget(current(SecureHostStatus.pending, true, false));
    expect(find.byKey(const ValueKey('secure-host-input')), findsNothing);
    await tester.pumpWidget(current(SecureHostStatus.active, false, false));
    expect(find.byKey(const ValueKey('secure-host-input')), findsNothing);
    await tester.pumpWidget(current(SecureHostStatus.active, true, false));
    expect(find.byKey(const ValueKey('secure-host-input')), findsNothing);
    expect(find.text('控制端已关闭'), findsOneWidget);
    expect(find.text('允许键鼠'), findsNothing);
    expect(find.text('撤销键鼠'), findsNothing);
    await tester.pumpWidget(current(SecureHostStatus.active, true, true));
    expect(find.text('控制端已开启'), findsOneWidget);
    expect(find.text('撤销键鼠'), findsNothing);
    await tester.tap(find.byKey(const ValueKey('secure-host-disconnect')));
    expect(disconnected, [9]);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'approval targets the selected request and waits for confirmation',
      (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await setWindow(tester, const Size(1120, 760));
    final approvals = <int>[];
    await tester.pumpWidget(workspace(
      connections: [connection(7, SecureHostStatus.pending)],
      selectedId: 7,
      onApprove: approvals.add,
    ));

    await tester.tap(find.byKey(const ValueKey('secure-host-approve')));
    expect(approvals, [7]);
    expect(find.text('本次连接已批准'), findsNothing);

    await tester.pumpWidget(workspace(
      connections: [connection(7, SecureHostStatus.confirming)],
      selectedId: 7,
      onApprove: approvals.add,
    ));
    final approving = tester.widget<FilledButton>(
      find.byKey(const ValueKey('secure-host-approve')),
    );
    expect(approving.onPressed, isNull);
    expect(find.text('正在确认…'), findsOneWidget);
    expect(find.text('本次连接已批准'), findsNothing);

    await tester.pumpWidget(workspace(
      connections: [connection(7, SecureHostStatus.active)],
      selectedId: 7,
    ));
    expect(find.text('本次连接已批准'), findsOneWidget);
    expect(find.byKey(const ValueKey('secure-host-approve')), findsNothing);
    expect(find.text('静态范围示意，不提供实时画面反馈'), findsOneWidget);
    expect(find.text('需视频启用并协商'), findsOneWidget);
    expect(find.text('本次已允许'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'selection and close actions retain IDs and unavailable tools stay disabled',
      (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await setWindow(tester, const Size(1120, 760));
    final selected = <int>[];
    final rejected = <int>[];
    final disconnected = <int>[];
    final clients = [
      connection(7, SecureHostStatus.pending),
      connection(9, SecureHostStatus.active),
    ];
    await tester.pumpWidget(workspace(
      connections: clients,
      selectedId: 7,
      onSelect: selected.add,
      onReject: rejected.add,
    ));
    await tester.tap(find.byKey(const ValueKey('secure-host-reject')));
    expect(rejected, [7]);
    await tester.tap(find.byKey(const ValueKey('secure-host-connection-9')));
    expect(selected, [9]);

    await tester.pumpWidget(workspace(
      connections: clients,
      selectedId: 9,
      onDisconnect: disconnected.add,
    ));
    await tester.tap(find.byKey(const ValueKey('secure-host-disconnect')));
    expect(disconnected, [9]);
    await tester.pumpWidget(workspace(
      connections: [connection(9, SecureHostStatus.ended)],
      selectedId: 9,
    ));
    expect(find.byKey(const ValueKey('secure-host-approve')), findsNothing);
    expect(find.byKey(const ValueKey('secure-host-disconnect')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('secure-host-nav-settings')));
    await tester.pump();
    for (final capability in [
      'invite',
      'files',
      'clipboard',
      'elevation',
    ]) {
      final button = tester.widget<OutlinedButton>(find.byKey(
        ValueKey('secure-host-capability-$capability'),
      ));
      expect(button.onPressed, isNull, reason: capability);
    }
    expect(
      tester
          .widget<OutlinedButton>(
              find.byKey(const ValueKey('secure-host-copy-id')))
          .onPressed,
      isNull,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'wide narrow and short windows keep session actions visible without overflow',
      (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    for (final size in const [
      Size(1120, 760),
      Size(840, 620),
      Size(840, 420)
    ]) {
      await setWindow(tester, size);
      for (final status in [
        SecureHostStatus.pending,
        SecureHostStatus.active
      ]) {
        await tester.pumpWidget(workspace(
          connections: [connection(7, status)],
          selectedId: 7,
        ));
        await tester.pump();
        final actions = status == SecureHostStatus.pending
            ? ['approve', 'reject']
            : ['disconnect'];
        for (final action in actions) {
          final finder = find.byKey(ValueKey('secure-host-$action'));
          expect(finder.hitTestable(), findsOneWidget,
              reason: '$action at $size');
          final rect = tester.getRect(finder);
          expect(rect.top, greaterThanOrEqualTo(0), reason: '$action at $size');
          expect(rect.bottom, lessThanOrEqualTo(size.height),
              reason: '$action at $size');
        }
        expect(tester.takeException(), isNull, reason: '$status at $size');
        await tester.drag(
            find.byKey(const ValueKey('secure-host-content-scroll')),
            const Offset(0, -500));
        await tester.pumpAndSettle();
        for (final action in actions) {
          expect(find.byKey(ValueKey('secure-host-$action')).hitTestable(),
              findsOneWidget);
        }
        expect(tester.takeException(), isNull,
            reason: 'scrolled $status at $size');
      }
    }
  });
}
