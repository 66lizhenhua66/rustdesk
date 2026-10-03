const secureHostArgument = '--ord-secure-ui';

// Set after the native library confirms its strict host build, before ServerModel starts.
bool isSecureHostWindow = false;

bool requestsSecureHostWindow(List<String> args) =>
    args.isNotEmpty &&
    args.first == '--cm' &&
    args.skip(1).contains(secureHostArgument);
